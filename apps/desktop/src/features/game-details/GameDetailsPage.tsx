// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { ImagePlus, Loader2, Pencil, RefreshCw, Trash2 } from "lucide-react";
import { DayBars, DayBarsLegend } from "@/components/charts/DayBars";
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
import { SessionLine } from "@/features/sessions/components/SessionLine";
import { ACTIVITY_CHART_DAYS, EVENT_LOG_LIMIT, GAME_RECENT_SESSIONS } from "@/lib/constants";
import { formatIntegrityEventType, getIntegrityEventDetail } from "@/lib/integrity";
import { gamePlaytime } from "@/lib/sentences";
import { buildDailyActivity } from "@/lib/session-stats";
import * as api from "@/lib/tauri";
import { formatCalendarDay, formatHoursMinutes, formatSessionStart } from "@/lib/time";
import type { Game, GameAssetView, SessionEvent } from "@/lib/types";
import { capitalize, numberWords } from "@/lib/words";
import { cn } from "@/lib/utils";

const SOURCE_LABELS: Record<string, string> = {
  steam: "Steam",
  folder_scan: "Found in a folder",
};

export function GameDetailsPage() {
  const { gameId } = useParams();
  // A fresh page per game, so nothing from the last game lingers while loading.
  return gameId ? <GamePage key={gameId} gameId={gameId} /> : null;
}

function GamePage({ gameId }: { gameId: string }) {
  const navigate = useNavigate();
  const { summaries, sessions: allSessions, active, loaded, refresh } = useLibrary();
  const [events, setEvents] = useState<SessionEvent[]>([]);
  const [assets, setAssets] = useState<GameAssetView[]>([]);
  const [showAll, setShowAll] = useState(false);
  const [editing, setEditing] = useState<Game | null>(null);
  const [deleting, setDeleting] = useState<Game | null>(null);

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
  const firstPlayed = sessions.at(-1)?.started_at_wall;
  const longest = sessions.reduce((best, session) => Math.max(best, session.runtime_ms), 0);
  const idleMs = sessions.reduce((sum, session) => sum + session.idle_ms, 0);

  function onAssetsChanged(next: GameAssetView[]) {
    setAssets(next);
    // Covers and tints live in the library state.
    refresh().catch(() => {});
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
                  <span className="size-2 animate-live-ring rounded-full bg-violet" />
                  <span>Playing now</span>
                </>
              )}
            </TintedOverline>
            <TintedTitle tint={tint} text={game.title} />
            <TintedSentence tint={tint}>
              <PhraseText phrase={gamePlaytime(sessions)} />
            </TintedSentence>
          </div>
          <Cover
            title={game.title}
            src={cover}
            variant="card"
            className="hidden h-[240px] w-[180px] p-4 text-[30px] shadow-2xl shadow-black/40 lg:flex"
          />
        </div>
      </TintedHeader>

      <div className="grid xl:grid-cols-[minmax(0,1fr)_340px]">
        <div className="flex min-w-0 flex-col gap-10 px-8 pt-8 xl:pr-10 xl:pl-14">
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
            <h2 id="sessions-title" className="font-display mb-1.5 text-[30px]">
              {showAll ? "Every session" : "Recent sessions"}
            </h2>
            {sessions.length === 0 ? (
              <p className="py-3.5 text-faint">No sessions yet. They appear here as soon as you play.</p>
            ) : (
              shown.map((session) => <SessionLine key={session.id} session={session} events={events} />)
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
            <TotalRow label="Runtime" value={formatHoursMinutes(summary.runtimeMs)} />
            <TotalRow label="Active" value={formatHoursMinutes(summary.activeMs)} accent />
            <TotalRow label="Idle" value={formatHoursMinutes(idleMs)} />
            <TotalRow label="Sessions" value={String(summary.sessionsCount)} />
            <TotalRow label="Longest" value={formatHoursMinutes(longest)} />
            <TotalRow label="First played" value={firstPlayed ? formatCalendarDay(firstPlayed) : "Not yet"} />
          </AsideSection>

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
                No executable set, so this game is not tracked yet. Edit it to pick one.
              </p>
            )}
            <div className="mt-4 flex gap-2">
              <Button variant="outline" size="sm" onClick={() => setEditing(game)}>
                <Pencil className="size-3.5" />
                Edit
              </Button>
              <Button variant="ghost" size="sm" className="text-faint" onClick={() => setDeleting(game)}>
                <Trash2 className="size-3.5" />
                Delete
              </Button>
            </div>
          </AsideSection>
        </aside>
      </div>

      <EditGameDialog game={editing} onClose={() => setEditing(null)} onSaved={refresh} />
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
      setError(`${failure}: ${String(taskError)}`);
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
            <span className="w-[132px] shrink-0 font-mono text-faint">{formatSessionStart(event.event_time_wall)}</span>
            <span className="w-40 shrink-0 text-soft">{formatIntegrityEventType(event.event_type)}</span>
            <span className="min-w-0 flex-1 text-faint">{getIntegrityEventDetail(event)}</span>
          </li>
        ))}
      </ul>
    </details>
  );
}
