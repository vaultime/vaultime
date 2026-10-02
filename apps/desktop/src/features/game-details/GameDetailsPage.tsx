// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Eye, EyeOff, ImagePlus, Loader2, Pencil, Plus, RefreshCw, Trash2 } from "lucide-react";
import { DayBars, DayBarsLegend } from "@/components/charts/DayBars";
import { Notice } from "@/components/layout/Page";
import {
  TintedHeader,
  TintedOverline,
  TintedSentence,
  TintedTitle,
} from "@/components/layout/Tinted";
import { Cover } from "@/components/media/Cover";
import { PhraseText } from "@/components/media/PhraseText";
import { Button } from "@/components/ui/button";
import { DeleteGameDialog } from "@/features/library/components/DeleteGameDialog";
import { EditGameDialog } from "@/features/library/components/EditGameDialog";
import { useLibrary } from "@/features/library/library-context";
import { AddSessionDialog } from "@/features/game-details/AddSessionDialog";
import { StatusPicker } from "@/features/game-details/StatusPicker";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import { ACTIVITY_CHART_DAYS, EVENT_LOG_LIMIT, GAME_RECENT_SESSIONS, MINUTE_MS, STEAM_LAUNCHER } from "@/lib/constants";
import { formatIntegrityEventType, getIntegrityEventDetail } from "@/lib/integrity";
import { gamePlaytime } from "@/lib/sentences";
import { buildDailyActivity, countsAsPlay } from "@/lib/session-stats";
import * as api from "@/lib/tauri";
import { formatCalendarDay, formatHoursMinutes, formatSessionStart } from "@/lib/time";
import type { EarlierPlaytime, Game, GameAssetView, SessionEvent } from "@/lib/types";
import { capitalize, numberWords } from "@/lib/words";
import { cn, describeError } from "@/lib/utils";

const SOURCE_LABELS: Record<string, string> = {
  steam: "Steam",
  epic: "Epic Games",
  battlenet: "Battle.net",
  riot: "Riot",
  hoyoplay: "HoYoPlay",
  ubisoft: "Ubisoft Connect",
  ea: "EA app",
  rockstar: "Rockstar",
  xbox: "Xbox",
  amazon: "Amazon Games",
  itch: "itch.io",
  gog: "GOG",
  heroic: "Heroic",
  lutris: "Lutris",
  folder_scan: "Found in a folder",
};

export function GameDetailsPage() {
  const { gameId } = useParams();
  // A fresh page per game, so nothing from the last game lingers while loading.
  return gameId ? <GamePage key={gameId} gameId={gameId} /> : null;
}

function GamePage({ gameId }: { gameId: string }) {
  const navigate = useNavigate();
  const { summaries, sessions: allSessions, active, loaded, refresh, notes, saveNote, setStatus } = useLibrary();
  const [events, setEvents] = useState<SessionEvent[]>([]);
  const [assets, setAssets] = useState<GameAssetView[]>([]);
  const [showAll, setShowAll] = useState(false);
  const [addingSession, setAddingSession] = useState(false);
  const [editing, setEditing] = useState<Game | null>(null);
  const [deleting, setDeleting] = useState<Game | null>(null);
  const [hideError, setHideError] = useState<string | null>(null);

  const summary = summaries.find((entry) => entry.game.id === gameId);
  const sessions = useMemo(
    () =>
      allSessions
        .filter((session) => session.game_id === gameId)
        .sort((a, b) => b.started_at_wall.localeCompare(a.started_at_wall)),
    [allSessions, gameId],
  );
  const days = useMemo(() => buildDailyActivity(sessions, ACTIVITY_CHART_DAYS), [sessions]);
  const playing = active.some((session) => session.game_id === gameId);

  // Events explain flagged sessions. Reload them whenever the sessions change.
  useEffect(() => {
    let cancelled = false;
    api
      .getSessionEventsForGame(gameId)
      .then((next) => {
        if (!cancelled) setEvents(next);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [gameId, sessions]);

  useEffect(() => {
    let cancelled = false;
    api
      .listGameAssets(gameId)
      .then((next) => {
        if (!cancelled) setAssets(next);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [gameId]);

  if (!loaded) return null;

  if (!summary) {
    return (
      <div className="px-8 pt-12 xl:px-14">
        <h1 className="font-display text-4xl">This game is not in your library</h1>
        <p className="mt-3 text-faint">It may have been deleted.</p>
        <Button variant="outline" className="mt-6" nativeButton={false} render={<Link to="/library" />}>
          Back to the library
        </Button>
      </div>
    );
  }

  const { game, cover, tint } = summary;
  const shown = showAll ? sessions : sessions.slice(0, GAME_RECENT_SESSIONS);
  const firstPlayed = sessions.filter(countsAsPlay).at(-1)?.started_at_wall;
  const longest = sessions.reduce((best, session) => Math.max(best, session.runtime_ms), 0);
  const idleMs = sessions.reduce((sum, session) => sum + session.idle_ms, 0);

  function onAssetsChanged(next: GameAssetView[]) {
    setAssets(next);
    // Covers and tints live in the library state.
    refresh().catch(() => {});
  }

  async function setHidden(hidden: boolean) {
    try {
      setHideError(null);
      await api.updateGame(game.id, { is_hidden: hidden });
      await refresh();
    } catch (error) {
      setHideError(describeError(error));
    }
  }

  return (
    <div className="pb-16">
      <TintedHeader tint={tint} backdrop={cover} className="pt-9 pb-9">
        <div className="flex items-end gap-10">
          <div className="min-w-0 flex-1">
            <TintedOverline tint={tint}>
              <Link to="/library" className="hover:underline">
                Library
              </Link>
              <span aria-hidden="true">/</span>
              <span>{SOURCE_LABELS[game.launcher_source ?? ""] ?? "Added by hand"}</span>
              {playing && (
                <>
                  <span aria-hidden="true">/</span>
                  <span className="size-2 rounded-full bg-violet ring-3 ring-violet/25" />
                  <span>Playing now</span>
                </>
              )}
            </TintedOverline>
            <TintedTitle tint={tint} text={game.title} />
            {/* Room for two lines, so the header keeps its height from game to game. */}
            <TintedSentence tint={tint} className="min-h-[2.6em]">
              <PhraseText phrase={gamePlaytime(sessions)} />
            </TintedSentence>
            <div className="mt-6">
              <StatusPicker
                tint={tint}
                status={summary.status}
                since={summary.statusSince}
                onChange={(status) => void setStatus(game.id, status).catch(() => {})}
              />
            </div>
          </div>
          <Cover
            title={game.title}
            src={cover}
            variant="card"
            className="hidden h-[240px] w-[180px] p-4 text-[30px] shadow-2xl shadow-scrim/40 lg:flex"
          />
        </div>
      </TintedHeader>

      <div className="grid xl:grid-cols-[minmax(0,1fr)_340px]">
        <div className="flex min-w-0 flex-col gap-10 px-8 pt-8 xl:pr-10 xl:pl-14">
          {(game.is_hidden || hideError) && (
            <div className="-mb-4 flex flex-col gap-3">
              {game.is_hidden && (
                <Notice>
                  Hidden from the library. Vaultime still tracks it, and its sessions count in the journal and your
                  totals.
                  <Button variant="outline" size="sm" onClick={() => void setHidden(false)}>
                    <Eye className="size-3.5" />
                    Show in library
                  </Button>
                </Notice>
              )}
              {hideError && <Notice tone="warning">{hideError}</Notice>}
            </div>
          )}

          <section aria-labelledby="days-title" className="flex flex-col gap-3.5">
            <div className="flex items-baseline justify-between">
              <h2 id="days-title" className="font-display text-[30px]">
                The last {numberWords(ACTIVITY_CHART_DAYS)} days
              </h2>
              <DayBarsLegend />
            </div>
            <DayBars points={days} />
          </section>

          <section aria-labelledby="sessions-title">
            <div className="mb-1.5 flex items-baseline justify-between gap-4">
              <h2 id="sessions-title" className="font-display text-[30px]">
                {showAll ? "Every session" : "Recent sessions"}
              </h2>
              <Button variant="ghost" size="sm" className="-mr-3.5 text-faint" onClick={() => setAddingSession(true)}>
                <Plus className="size-3.5" />
                Add a session
              </Button>
            </div>
            {sessions.length === 0 ? (
              <p className="py-3.5 text-faint">No sessions yet. They appear here as soon as you play.</p>
            ) : (
              shown.map((session) => (
                <SessionLine
                  key={session.id}
                  session={session}
                  events={events}
                  gameSessions={sessions}
                  earlierMs={summary.earlier?.earlier_ms}
                  note={notes[session.id]}
                  onSaveNote={(note) => saveNote(session.id, note)}
                  onCorrected={() => void refresh()}
                />
              ))
            )}
            {sessions.length > GAME_RECENT_SESSIONS && (
              <button
                type="button"
                onClick={() => setShowAll((value) => !value)}
                className="mt-4 text-sm text-violet hover:text-text focus-visible:underline focus-visible:outline-none"
              >
                {showAll ? "Show recent sessions only" : `Show all ${sessions.length} sessions`}
              </button>
            )}
          </section>

          <EventLog events={events} />
        </div>

        <aside
          aria-label="Details"
          className="flex flex-col gap-8 border-rule px-8 pt-10 xl:border-l xl:pt-8 xl:pr-14 xl:pl-8"
        >
          <AsideSection title="Totals">
            {summary.earlier && <TotalRow label="In all" value={formatHoursMinutes(summary.totalMs)} />}
            <TotalRow label={summary.earlier ? "Tracked here" : "Runtime"} value={formatHoursMinutes(summary.runtimeMs)} />
            <TotalRow label="Active" value={formatHoursMinutes(summary.activeMs)} accent />
            <TotalRow label="Idle" value={formatHoursMinutes(idleMs)} />
            <TotalRow label="Sessions" value={String(summary.sessionsCount)} />
            <TotalRow label="Suspicious" value={String(summary.suspiciousCount)} />
            <TotalRow label="Recovered" value={String(summary.recoveredCount)} />
            <TotalRow label="Longest" value={formatHoursMinutes(longest)} />
            <TotalRow label="First played" value={firstPlayed ? formatCalendarDay(firstPlayed) : "Not yet"} />
          </AsideSection>

          {summary.earlier && <EarlierSection earlier={summary.earlier} />}

          <CoverPicker gameId={gameId} assets={assets} onChanged={onAssetsChanged} />

          <AsideSection title="Tracking">
            {game.executable_path ? (
              <>
                <div className="font-mono text-xs leading-relaxed break-all text-soft">{game.executable_path}</div>
                <p className="mt-2 text-[13px] text-faint">
                  Matched by its full path. By file name only when Windows hides the path or the game runs through Wine.
                </p>
              </>
            ) : (
              <p className="text-[13px] text-faint">
                No program set, so this game is not tracked yet. Edit it to pick one.
              </p>
            )}
            <div className="mt-4 flex flex-wrap gap-2">
              <Button variant="outline" size="sm" onClick={() => setEditing(game)}>
                <Pencil className="size-3.5" />
                Edit
              </Button>
              <Button variant="ghost" size="sm" className="text-faint" onClick={() => void setHidden(!game.is_hidden)}>
                {game.is_hidden ? <Eye className="size-3.5" /> : <EyeOff className="size-3.5" />}
                {game.is_hidden ? "Unhide" : "Hide"}
              </Button>
              <Button
                variant="ghost"
                size="icon-sm"
                className="text-faint"
                aria-label="Delete"
                title="Delete"
                onClick={() => setDeleting(game)}
              >
                <Trash2 className="size-3.5" />
              </Button>
            </div>
          </AsideSection>
        </aside>
      </div>

      <EditGameDialog game={editing} onClose={() => setEditing(null)} onSaved={refresh} />
      {addingSession && (
        <AddSessionDialog
          gameId={game.id}
          gameTitle={game.title}
          steamGame={game.launcher_source === STEAM_LAUNCHER || summary.earlier?.source === STEAM_LAUNCHER}
          open={addingSession}
          onOpenChange={setAddingSession}
          onAdded={() => void refresh()}
        />
      )}
      <DeleteGameDialog
        game={deleting}
        onClose={() => setDeleting(null)}
        onDeleted={() => {
          navigate("/library");
          refresh().catch(() => {});
        }}
      />
    </div>
  );
}

function AsideSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="label-caps mb-2.5">{title}</h2>
      {children}
    </section>
  );
}

/** Playtime from before Vaultime, and what it is made of. */
function EarlierSection({ earlier }: { earlier: EarlierPlaytime }) {
  const launcherMs = earlier.launcher_minutes * MINUTE_MS;
  return (
    <AsideSection title="Before Vaultime">
      <TotalRow label="From Steam" value={formatHoursMinutes(earlier.earlier_ms)} />
      {earlier.last_played_at && (
        <TotalRow label="Last played on Steam" value={formatCalendarDay(earlier.last_played_at)} />
      )}
      <p className="mt-2.5 text-[13px] leading-relaxed text-faint">
        Steam counted {formatHoursMinutes(launcherMs)} by {formatCalendarDay(earlier.imported_at)}
        {earlier.tracked_before_ms > 0
          ? `, of which ${formatHoursMinutes(Math.min(earlier.tracked_before_ms, launcherMs))} were tracked here too and count once`
          : ""}
        . It has no sessions, so the journal and the stats leave it out.
      </p>
    </AsideSection>
  );
}

function TotalRow({ label, value, accent = false }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="flex items-baseline justify-between border-b border-rule py-2.5">
      <span className="text-sm text-soft">{label}</span>
      <span className={cn("font-mono text-[15px] tabular-nums", accent && "text-violet")}>{value}</span>
    </div>
  );
}

function CoverPicker({
  gameId,
  assets,
  onChanged,
}: {
  gameId: string;
  assets: GameAssetView[];
  onChanged: (assets: GameAssetView[]) => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const withPreview = assets.filter((asset) => asset.preview_data_url);

  async function run(task: () => Promise<GameAssetView[] | null>, failure: string) {
    try {
      setBusy(true);
      setError(null);
      const next = await task();
      if (next) onChanged(next);
    } catch (taskError) {
      setError(`${failure}: ${describeError(taskError)}`);
    } finally {
      setBusy(false);
    }
  }

  const rescan = () => run(() => api.scanGameAssets(gameId), "The scan failed");
  const choose = (assetId: string) =>
    run(async () => {
      await api.setPreferredGameAsset(gameId, assetId);
      return api.listGameAssets(gameId);
    }, "Could not use that image");
  const addImage = () =>
    run(async () => {
      const selected = await openFileDialog({
        multiple: false,
        directory: false,
        title: "Choose a cover image",
        filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "webp", "bmp", "ico"] }],
      });
      return selected ? api.importGameAsset(gameId, selected) : null;
    }, "Could not add the image");

  return (
    <AsideSection title="Cover">
      {withPreview.length > 0 && (
        <div className="flex flex-wrap gap-2.5">
          {withPreview.map((asset, index) => (
            <button
              key={asset.id}
              type="button"
              disabled={busy}
              onClick={() => choose(asset.id)}
              aria-pressed={asset.is_preferred}
              aria-label={`Use image ${index + 1}${asset.is_preferred ? ", in use" : ""}`}
              className={cn(
                "h-24 w-[72px] overflow-hidden rounded-[5px] border transition focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:outline-none",
                asset.is_preferred ? "border-2 border-text" : "border-rule opacity-70 hover:opacity-100",
              )}
            >
              <img src={asset.preview_data_url ?? undefined} alt="" className="size-full object-cover" />
            </button>
          ))}
        </div>
      )}
      <p className="mt-2.5 text-[13px] text-faint">
        {withPreview.length === 0
          ? "No artwork yet. Scan the game folder or add an image."
          : `${capitalize(numberWords(withPreview.length))} image${withPreview.length === 1 ? "" : "s"} to choose from.`}
      </p>
      <div className="mt-3 flex gap-2">
        <Button variant="outline" size="sm" onClick={rescan} disabled={busy}>
          {busy ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
          Scan folder
        </Button>
        <Button variant="outline" size="sm" onClick={addImage} disabled={busy}>
          <ImagePlus className="size-3.5" />
          Add image
        </Button>
      </div>
      {error && (
        <p role="alert" className="mt-2.5 text-[13px] text-amber">
          {error}
        </p>
      )}
    </AsideSection>
  );
}

/** The notable local events, for anyone who wants to check the record. */
function EventLog({ events }: { events: SessionEvent[] }) {
  const notable = events.filter((event) => event.event_type !== "heartbeat");
  if (notable.length === 0) return null;
  const shown = [...notable]
    .sort((a, b) => b.event_time_wall.localeCompare(a.event_time_wall))
    .slice(0, EVENT_LOG_LIMIT);

  return (
    <details className="group border-t border-rule pt-4">
      <summary className="flex cursor-pointer list-none items-baseline justify-between text-sm text-faint hover:text-soft">
        <span>
          Event log, {notable.length} entr{notable.length === 1 ? "y" : "ies"}
        </span>
        <span className="text-xs group-open:hidden">Show</span>
        <span className="hidden text-xs group-open:inline">Hide</span>
      </summary>
      <p className="mt-3 text-[13px] text-faint">
        Every session keeps a hash-linked log on this PC. It can reveal edits to the record, but it cannot
        prove there were none.
      </p>
      <ul className="mt-2">
        {shown.map((event) => (
          <li key={event.id} className="flex items-baseline gap-5 border-b border-rule py-2.5 text-[13px]">
            <span className="w-[20ch] shrink-0 font-mono text-faint">{formatSessionStart(event.event_time_wall)}</span>
            <span className="w-40 shrink-0 text-soft">{formatIntegrityEventType(event.event_type)}</span>
            <span className="min-w-0 flex-1 text-faint">{getIntegrityEventDetail(event)}</span>
          </li>
        ))}
      </ul>
    </details>
  );
}
