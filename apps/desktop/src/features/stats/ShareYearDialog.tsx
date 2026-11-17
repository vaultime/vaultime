// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { save as saveFileDialog } from "@tauri-apps/plugin-dialog";
import { Check, Copy, Download, Eye, EyeOff, Loader2 } from "lucide-react";
import { Notice } from "@/components/layout/Page";
import { Cover } from "@/components/media/Cover";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useLibrary } from "@/features/library/library-context";
import { HOUR_MS, SHARE_COPIED_NOTE_MS, SHARE_FORMATS, SHARE_TOP_GAMES } from "@/lib/constants";
import { onRadioKeys, radioTabIndex } from "@/lib/radio-group";
import {
  buildShareCard,
  previewScale,
  SHARE_FORMAT_IDS,
  shareChoices,
  shareFileName,
  type ShareCard,
  type ShareChoice,
  type ShareFormat,
  type ShareGameInfo,
  type ShareOptions,
} from "@/lib/share-card";
import type { YearStats } from "@/lib/stats";
import * as api from "@/lib/tauri";
import type { ThemeMode } from "@/lib/theme";
import { formatHoursShort } from "@/lib/time";
import { cn, describeError } from "@/lib/utils";
import { plural } from "@/lib/words";
import { drawShareCard, type CardLook } from "./share-card-draw";

const FORMAT_NAMES: Record<ShareFormat, { label: string; hint: string }> = {
  wide: { label: "Wide", hint: "X, Discord, Mastodon" },
  square: { label: "Square", hint: "Feeds" },
  story: { label: "Story", hint: "Stories" },
};

const MODES: { value: ThemeMode; label: string }[] = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
];

/** Whether this webview can put a picture on the clipboard. */
function clipboardTakesImages(): boolean {
  if (typeof ClipboardItem === "undefined" || typeof navigator.clipboard?.write !== "function") return false;
  return typeof ClipboardItem.supports === "function" ? ClipboardItem.supports("image/png") : true;
}

function canvasBlob(canvas: HTMLCanvasElement): Promise<Blob> {
  return new Promise((resolve, reject) =>
    canvas.toBlob((blob) => (blob ? resolve(blob) : reject(new Error("the card could not be drawn"))), "image/png"),
  );
}

async function canvasBase64(canvas: HTMLCanvasElement): Promise<string> {
  const blob = await canvasBlob(canvas);
  const url = await new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(reader.error ?? new Error("the card could not be read"));
    reader.readAsDataURL(blob);
  });
  return url.slice(url.indexOf(",") + 1);
}

/** A card of the year to post, drawn on this PC. Nothing leaves it unless the player saves or copies the picture. */
export function ShareYearDialog({
  open,
  onOpenChange,
  stats,
  now,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  stats: YearStats;
  now: Date;
}) {
  const { summaries, covers } = useLibrary();
  // The year as it was when the dialog opened, so the card holds still while a game runs.
  const [held, setHeld] = useState<{ stats: YearStats; now: Date } | null>(null);
  if (open && held === null) setHeld({ stats, now });
  if (!open && held !== null) setHeld(null);
  const year = held ?? { stats, now };
  const { appearance, mode, accentHues, accent } = useAppearance();
  // The accent the app shows, so a look lit by a game draws the game's color, not grey.
  const look = useMemo(() => ({ ground: appearance.ground, accent }), [appearance.ground, accent]);
  const [format, setFormat] = useState<ShareFormat>("wide");
  const [showGames, setShowGames] = useState(true);
  const [showCalendar, setShowCalendar] = useState(true);
  const [showPcName, setShowPcName] = useState(false);
  const [leftOff, setLeftOff] = useState<ReadonlySet<string>>(() => new Set());
  const [cardMode, setCardMode] = useState<ThemeMode>(mode);
  const [pcName, setPcName] = useState<string | null>(null);
  const [busy, setBusy] = useState<"save" | "copy" | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const [canCopy, setCanCopy] = useState(clipboardTakesImages);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    api.getThisPc().then(
      (pc) => !cancelled && setPcName(pc.name),
      () => {},
    );
    return () => {
      cancelled = true;
    };
  }, [open]);

  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), SHARE_COPIED_NOTE_MS);
    return () => clearTimeout(timer);
  }, [copied]);

  const options: ShareOptions = { format, showGames, showCalendar, showPcName, mode: cardMode };
  const gameInfo = useMemo(() => {
    const byGame = new Map(summaries.map((summary) => [summary.game.id, summary]));
    return (gameId: string): ShareGameInfo | undefined => {
      const summary = byGame.get(gameId);
      if (!summary) return undefined;
      return {
        title: summary.game.title,
        hidden: summary.game.is_hidden,
        cover: covers[gameId] ?? summary.cover,
        colors: summary.tint.colors,
      };
    };
  }, [summaries, covers]);
  const card = useMemo(
    () =>
      buildShareCard({
        stats: year.stats,
        // A game left off counts as a hidden one: in the hours, never named.
        gameInfo: (gameId) => {
          const info = gameInfo(gameId);
          return info && leftOff.has(gameId) ? { ...info, hidden: true } : info;
        },
        pcName,
        accentHues,
        options: { showGames, showPcName, mode: cardMode },
        now: year.now,
      }),
    [year.stats, year.now, gameInfo, leftOff, pcName, accentHues, showGames, showPcName, cardMode],
  );
  const choices = useMemo(
    () => shareChoices(year.stats, gameInfo, SHARE_TOP_GAMES[format], leftOff),
    [year.stats, gameInfo, format, leftOff],
  );

  function toggleLeftOff(gameId: string) {
    setLeftOff((current) => {
      const next = new Set(current);
      if (!next.delete(gameId)) next.add(gameId);
      return next;
    });
  }

  /** The card for the options set now. The preview may still show the one before a change. */
  async function drawForExport(): Promise<HTMLCanvasElement> {
    if (!card) throw new Error("there is no card to draw");
    const target = document.createElement("canvas");
    await drawShareCard(target, card, options, look);
    return target;
  }

  function reset() {
    setMessage(null);
    setError(null);
  }

  async function save() {
    reset();
    try {
      const picked = await saveFileDialog({
        defaultPath: shareFileName(stats.year),
        // GTK on Linux matches extensions with their case.
        filters: [{ name: "PNG image", extensions: ["png", "PNG"] }],
      });
      if (!picked) return;
      const path = /\.png$/i.test(picked) ? picked : `${picked}.png`;
      setBusy("save");
      await api.saveShareImage(path, await canvasBase64(await drawForExport()));
      setMessage(`Saved to ${path}.`);
    } catch (saveError) {
      setError(describeError(saveError));
    } finally {
      setBusy(null);
    }
  }

  async function copy() {
    reset();
    setBusy("copy");
    try {
      // The blob goes in as a promise, so the click still counts as the reason to write.
      await navigator.clipboard.write([new ClipboardItem({ "image/png": drawForExport().then(canvasBlob) })]);
      setCopied(true);
    } catch {
      setCanCopy(false);
      setError("This system does not let Vaultime put a picture on the clipboard. Save it as a PNG instead.");
    } finally {
      setBusy(null);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="h-[min(760px,calc(100dvh-2rem))] w-[min(1180px,calc(100%-2rem))] max-w-none grid-cols-1 grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-none [&>[data-slot=dialog-close]]:z-20">
        <div className="grid min-h-0 grid-cols-[minmax(0,1fr)_300px]">
          <CardPreview card={card} options={options} look={look} year={stats.year}>
            <SidePanel
              year={stats.year}
              format={format}
              setFormat={setFormat}
              showGames={showGames}
              setShowGames={setShowGames}
              showCalendar={showCalendar}
              setShowCalendar={setShowCalendar}
              showPcName={showPcName}
              setShowPcName={setShowPcName}
              pcName={pcName}
              choices={choices}
              onToggleChoice={toggleLeftOff}
              cardMode={cardMode}
              setCardMode={setCardMode}
              empty={!card}
              actions={
                <>
                  <Button onClick={() => void save()} disabled={!card || busy !== null}>
                    {busy === "save" ? <Loader2 className="animate-spin" /> : <Download />}
                    Save as PNG
                  </Button>
                  {canCopy && (
                    <Button variant="outline" onClick={() => void copy()} disabled={!card || busy !== null}>
                      {copied ? <Check /> : busy === "copy" ? <Loader2 className="animate-spin" /> : <Copy />}
                      {copied ? "Copied" : "Copy image"}
                    </Button>
                  )}
                </>
              }
              message={message}
              error={error}
            />
          </CardPreview>
        </div>
      </DialogContent>
    </Dialog>
  );
}

/** The card at the size of its format, scaled down to fit, and the panel beside it. */
function CardPreview({
  card,
  options,
  look,
  year,
  children,
}: {
  card: ShareCard | null;
  options: ShareOptions;
  look: CardLook;
  year: number;
  children: ReactNode;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const [canvas, setCanvas] = useState<HTMLCanvasElement | null>(null);
  const [box, setBox] = useState({ width: 0, height: 0 });
  const [drawn, setDrawn] = useState(false);

  useEffect(() => {
    const element = frame.current;
    if (!element) return;
    const observer = new ResizeObserver(([entry]) =>
      setBox({ width: entry.contentRect.width, height: entry.contentRect.height }),
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, []);

  const { format, showGames, showCalendar, showPcName, mode } = options;
  useEffect(() => {
    if (!canvas || !card) return;
    let current = true;
    const target = document.createElement("canvas");
    void drawShareCard(target, card, { format, showGames, showCalendar, showPcName, mode }, look).then(() => {
      if (!current) return;
      // Drawn aside and copied over, so the preview never shows half a card.
      canvas.width = target.width;
      canvas.height = target.height;
      canvas.getContext("2d")?.drawImage(target, 0, 0);
      setDrawn(true);
    });
    return () => {
      current = false;
    };
  }, [canvas, card, format, showGames, showCalendar, showPcName, mode, look]);

  const { width, height } = SHARE_FORMATS[format];
  const scale = previewScale(format, box.width, box.height);
  const summary = card
    ? `${card.year} in play: ${Math.floor(card.activeMs / HOUR_MS)} hours of active play on ${plural(card.daysPlayed, "day")}.`
    : "";

  return (
    <>
      <div className="flex min-h-0 min-w-0 flex-col bg-ink p-6 short:p-4">
        <div ref={frame} className="flex min-h-0 flex-1 items-center justify-center">
          {card ? (
            <canvas
              ref={setCanvas}
              role="img"
              aria-label={summary}
              data-share-card={format}
              width={width}
              height={height}
              className={cn(
                "rounded-[6px] shadow-2xl shadow-scrim/50 transition-opacity",
                drawn ? "opacity-100" : "opacity-0",
              )}
              style={{ width: width * scale, height: height * scale }}
            />
          ) : (
            <div className="max-w-[420px] text-center">
              <p className="font-display text-2xl leading-tight">Nothing to share for {year} yet</p>
              <p className="font-prose mt-3 text-[19px] leading-snug text-soft">
                Once you play, your year fills in here, day by day.
              </p>
            </div>
          )}
        </div>
      </div>
      {children}
    </>
  );
}

function SidePanel({
  year,
  format,
  setFormat,
  showGames,
  setShowGames,
  showCalendar,
  setShowCalendar,
  showPcName,
  setShowPcName,
  pcName,
  choices,
  onToggleChoice,
  cardMode,
  setCardMode,
  empty,
  actions,
  message,
  error,
}: {
  year: number;
  format: ShareFormat;
  setFormat: (format: ShareFormat) => void;
  showGames: boolean;
  setShowGames: (show: boolean) => void;
  showCalendar: boolean;
  setShowCalendar: (show: boolean) => void;
  showPcName: boolean;
  setShowPcName: (show: boolean) => void;
  pcName: string | null;
  choices: ShareChoice[];
  onToggleChoice: (gameId: string) => void;
  cardMode: ThemeMode;
  setCardMode: (mode: ThemeMode) => void;
  empty: boolean;
  actions: ReactNode;
  message: string | null;
  error: string | null;
}) {
  return (
    <div className="flex min-h-0 flex-col border-l border-rule">
      <div className="min-h-0 flex-1 overflow-y-auto px-6 pb-5">
        {/* Stays on top, so the options scroll under it, not under the close button. */}
        <div className="sticky top-0 z-10 -mx-6 bg-surface px-6 pt-7 pb-2 short:pt-5">
          <div className="label-small">Stats, {year}</div>
        </div>
        <DialogTitle className="text-[28px]">Share your year</DialogTitle>
        <DialogDescription className="mt-2">
          A picture of {year} in play, made on this PC. Nothing is uploaded.
        </DialogDescription>

        {!empty && (
          <>
        <fieldset className="mt-6 min-w-0">
          <legend className="label-small">Format</legend>
          <div role="radiogroup" aria-label="Format" onKeyDown={onRadioKeys} className="mt-2.5">
            {SHARE_FORMAT_IDS.map((id, index) => {
              const checked = format === id;
              const { width, height } = SHARE_FORMATS[id];
              return (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={checked}
                  tabIndex={radioTabIndex(checked, index, true)}
                  onClick={() => setFormat(id)}
                  className="flex w-full items-center gap-3 border-b border-rule py-2.5 text-left last:border-b-0 focus-visible:outline-none [&:focus-visible>span:first-child]:ring-2 [&:focus-visible>span:first-child]:ring-violet/60"
                >
                  <span
                    aria-hidden="true"
                    className={cn(
                      "flex size-[18px] shrink-0 items-center justify-center rounded-full border transition-colors",
                      checked ? "border-violet" : "border-hairline-strong",
                    )}
                  >
                    {checked && <span className="size-2 rounded-full bg-violet" />}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-[15px] text-text">{FORMAT_NAMES[id].label}</span>
                    <span className="block text-[12px] text-faint">{FORMAT_NAMES[id].hint}</span>
                  </span>
                  <span className="shrink-0 text-[12px] text-faint tabular-nums">
                    {width} × {height}
                  </span>
                </button>
              );
            })}
          </div>
        </fieldset>

        <fieldset className="mt-5 min-w-0">
          <legend className="label-small">On the card</legend>
          <div className="mt-1.5">
            <Toggle id="share-games" label="Game titles and covers" checked={showGames} onChange={setShowGames} hint={showGames ? undefined : "Shows how many games and how the time split, no names."} />
            <Toggle id="share-calendar" label="Calendar of the year" checked={showCalendar} onChange={setShowCalendar} />
            <Toggle
              id="share-pc"
              label="Name of this PC"
              hint={pcName ?? undefined}
              checked={showPcName}
              onChange={setShowPcName}
            />
          </div>
        </fieldset>

        {showGames && choices.length > 0 && (
          <fieldset className="mt-5 min-w-0">
            <legend className="label-small">Games on the card</legend>
            <ul className="mt-1.5">
              {choices.map((choice) => (
                <li key={choice.gameId} className="flex items-center gap-3 border-b border-rule py-2 last:border-b-0">
                  <Cover
                    title={choice.title}
                    src={choice.cover}
                    variant="tile"
                    className="h-8 w-6 rounded-[3px] text-[9px]"
                  />
                  <span className={cn("min-w-0 flex-1 truncate text-[14px]", choice.left ? "text-faint" : "text-text")}>
                    {choice.title}
                  </span>
                  <span className="shrink-0 text-[12px] text-faint tabular-nums">
                    {choice.left ? "Left off" : formatHoursShort(choice.activeMs)}
                  </span>
                  <button
                    type="button"
                    aria-pressed={!choice.left}
                    aria-label={`Show ${choice.title} on the card`}
                    title={choice.left ? "Put back on the card" : "Leave off the card"}
                    onClick={() => onToggleChoice(choice.gameId)}
                    className="flex size-7 shrink-0 items-center justify-center rounded-md text-faint transition-colors hover:bg-glint/6 hover:text-text focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
                  >
                    {choice.left ? <EyeOff className="size-4" strokeWidth={1.75} /> : <Eye className="size-4" strokeWidth={1.75} />}
                  </button>
                </li>
              ))}
            </ul>
            <p className="mt-2 text-[12px] leading-relaxed text-faint">
              A game left off still counts in the hours, it just goes unnamed.
            </p>
          </fieldset>
        )}

        <fieldset className="mt-5 min-w-0">
          <legend className="label-small">Look</legend>
          <div
            role="radiogroup"
            aria-label="Look of the card"
            onKeyDown={onRadioKeys}
            className="segmented mt-2.5"
          >
            {MODES.map((option, index) => (
              <button
                key={option.value}
                type="button"
                role="radio"
                aria-checked={cardMode === option.value}
                tabIndex={radioTabIndex(cardMode === option.value, index, true)}
                onClick={() => setCardMode(option.value)}
                className="segment"
              >
                {option.label}
              </button>
            ))}
          </div>
          <p className="mt-2 text-[12px] leading-relaxed text-faint">In your ground and accent colors.</p>
        </fieldset>
          </>
        )}

        {message && (
          <Notice className="mt-5 break-all">
            {message}
          </Notice>
        )}
        {error && (
          <Notice tone="warning" className="mt-5">
            {error}
          </Notice>
        )}
      </div>
      {!empty && <div className="flex flex-col gap-2 border-t border-rule px-6 py-4 [&>button]:w-full">{actions}</div>}
    </div>
  );
}

function Toggle({
  id,
  label,
  hint,
  checked,
  onChange,
}: {
  id: string;
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
}) {
  return (
    <div className="flex items-center justify-between gap-4 border-b border-rule py-2.5 last:border-b-0">
      <label htmlFor={id} className="min-w-0 cursor-pointer">
        <span className="block text-[14px] text-text">{label}</span>
        {hint && <span className="block text-[12px] leading-snug text-pretty text-faint [overflow-wrap:anywhere]">{hint}</span>}
      </label>
      <Switch id={id} checked={checked} onCheckedChange={onChange} />
    </div>
  );
}
