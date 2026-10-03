// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Crop, Eye, EyeOff, ImagePlus, Loader2, Pencil, Plus, RefreshCw, Trash2, X } from "lucide-react";
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
import { Switch } from "@/components/ui/switch";
import { DeleteGameDialog } from "@/features/library/components/DeleteGameDialog";
import { EditGameDialog } from "@/features/library/components/EditGameDialog";
import { GameLinkPicker } from "@/features/library/components/GameLinkPicker";
import { useLibrary } from "@/features/library/library-context";
import { AddSessionDialog } from "@/features/game-details/AddSessionDialog";
import { CoverCropDialog, type CropTarget } from "@/features/game-details/CoverCropDialog";
import { GameHistory } from "@/features/game-details/GameHistory";
import { StatusPicker } from "@/features/game-details/StatusPicker";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import {
  ARTWORK_DIALOG_EXTENSIONS,
  EVENT_LOG_LIMIT,
  FOLDER_MATCH_LAUNCHERS,
  GAME_RECENT_SESSIONS,
  MINUTE_MS,
  PLAYER_ARTWORK_SOURCE,
  STEAM_LAUNCHER,
  STEPS_ASIDE_HINT_MIN_MS,
  STEPS_ASIDE_HINT_MIN_OPENED_FIRST_SHARE,
  STEPS_ASIDE_HINT_MIN_SHARE,
} from "@/lib/constants";
import { pcName } from "@/lib/devices";
import { formatIntegrityEventType, getIntegrityEventDetail } from "@/lib/integrity";
import { gamePlaytime } from "@/lib/sentences";
import { countsAsPlay } from "@/lib/session-stats";
import { besideOthers, stepsAside } from "@/lib/steps-aside";
import * as api from "@/lib/tauri";
import { formatCalendarDay, formatHoursMinutes, formatSessionStart } from "@/lib/time";
import type { EarlierPlaytime, Game, GameAssetView, Session, SessionEvent } from "@/lib/types";
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
  const [steppingError, setSteppingError] = useState<string | null>(null);

  const summary = summaries.find((entry) => entry.game.id === gameId);
  const sessions = useMemo(
    () =>
      allSessions
        .filter((session) => session.game_id === gameId)
        .sort((a, b) => b.started_at_wall.localeCompare(a.started_at_wall)),
    [allSessions, gameId],
  );
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
  const stepping = new Set(summaries.filter((entry) => stepsAside(entry.game)).map((entry) => entry.game.id));
  const shown = showAll ? sessions : sessions.slice(0, GAME_RECENT_SESSIONS);
  const firstPlayed = sessions.filter(countsAsPlay).at(-1)?.started_at_wall;
  const longest = sessions.reduce((best, session) => Math.max(best, session.runtime_ms), 0);
  const idleMs = sessions.reduce((sum, session) => sum + session.idle_ms, 0);

  function onAssetsChanged(next: GameAssetView[]) {
    setAssets(next);
    // Covers and tints live in the library state.
    refresh().catch(() => {});
  }

  async function setStepping(next: boolean) {
    try {
      setSteppingError(null);
      await api.setGameStepsAside(game.id, next);
      await refresh();
    } catch (error) {
      setSteppingError(describeError(error));
    }
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

          <GameHistory gameId={game.id} sessions={sessions} tint={tint} title={game.title} />

          <section aria-labelledby="sessions-title">
            <div className="mb-1.5 flex items-baseline justify-between gap-4">
              <h2 id="sessions-title" className="font-display text-[30px]">
                {showAll ? "Every session" : "Recent sessions"}
              </h2>
              {/* A game of another PC gets its sessions with each merge. */}
              {!game.origin_device_id && (
                <Button variant="ghost" size="sm" className="-mr-3.5 text-faint" onClick={() => setAddingSession(true)}>
                  <Plus className="size-3.5" />
                  Add a session
                </Button>
              )}
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

          <CoverPicker
            gameId={gameId}
            gameTitle={game.title}
            assets={assets}
            onChanged={onAssetsChanged}
            scannable={!game.origin_device_id}
          />

          <AsideSection title="Tracking">
            {game.origin_device_id ? (
              <ForeignGameRows game={game} />
            ) : game.executable_path ? (
              <>
                <div className="font-mono text-xs leading-relaxed break-all text-soft">{game.executable_path}</div>
                <p className="mt-2 text-[13px] text-faint">
                  {game.install_folder && FOLDER_MATCH_LAUNCHERS.includes(game.launcher_source ?? "")
                    ? "Matched by its full path, or by any game program in its install folder, since its launcher starts games in steps."
                    : "Matched by its full path."}{" "}
                  By file name only when Windows hides the path or the game runs through Wine.
                </p>
              </>
            ) : (
              <p className="text-[13px] text-faint">
                No program set, so this game is not tracked yet. Edit it to pick one.
              </p>
            )}
            {!game.origin_device_id && <LinkedGameRows gameId={game.id} />}
            <StepsAsideRow
              game={game}
              sessions={allSessions}
              stepping={stepping}
              error={steppingError}
              onChange={(next) => void setStepping(next)}
            />
            <div className="mt-4 flex flex-wrap gap-2">
              {!game.origin_device_id && (
                <Button variant="outline" size="sm" onClick={() => setEditing(game)}>
                  <Pencil className="size-3.5" />
                  Edit
                </Button>
              )}
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

/**
 * The switch that makes a game count only while no other game runs, with a
 * hint when other games often ran beside it, like a launcher left open.
 */
function StepsAsideRow({
  game,
  sessions,
  stepping,
  error,
  onChange,
}: {
  game: Game;
  sessions: Session[];
  /** Games that count only while no other game runs. */
  stepping: Set<string>;
  error: string | null;
  onChange: (next: boolean) => void;
}) {
  const on = stepping.has(game.id);
  const { besideMs, share, openedFirstShare } = besideOthers(game.id, sessions, stepping);
  const suggest =
    !on &&
    share >= STEPS_ASIDE_HINT_MIN_SHARE &&
    besideMs >= STEPS_ASIDE_HINT_MIN_MS &&
    openedFirstShare >= STEPS_ASIDE_HINT_MIN_OPENED_FIRST_SHARE;
  return (
    <div className="mt-4 border-t border-rule pt-4">
      <div className="flex items-start justify-between gap-4">
        <label htmlFor="steps-aside" className="min-w-0">
          <span className="block text-[13px] text-text">Count only when no other game runs</span>
          <span className="mt-1 block text-[12px] leading-relaxed text-faint">
            For launchers and game clients. Its time beside another game is set aside, and switching this off brings
            that time back.
          </span>
        </label>
        <Switch id="steps-aside" checked={on} onCheckedChange={onChange} />
      </div>
      {suggest && (
        <p className="mt-2 text-[12px] leading-relaxed text-soft">
          Games started while it was open and ran beside it for {formatHoursMinutes(besideMs)},{" "}
          {Math.round(share * 100)} % of its time. If it is a launcher, this stops that time from counting for it.
        </p>
      )}
      {error && <p className="mt-2 text-[12px] text-amber">{error}</p>}
    </div>
  );
}

/** A game that came with sessions of another PC: where from, and which game of this PC it counts as. */
function ForeignGameRows({ game }: { game: Game }) {
  const { games, devices, refresh } = useLibrary();
  const [error, setError] = useState<string | null>(null);
  const ours = games.filter((candidate) => candidate.origin_device_id === null);
  const from = pcName(devices, game.origin_device_id ?? "");

  async function link(linkedGameId: string | null) {
    setError(null);
    try {
      await api.linkGame(game.id, linkedGameId);
      await refresh();
    } catch (linkError) {
      setError(describeError(linkError));
    }
  }

  return (
    <div>
      <p className="text-[13px] leading-relaxed text-faint">
        Came with the sessions of {from}. Not tracked on this PC, its sessions come with each merge.
      </p>
      <div className="mt-3 flex items-center justify-between gap-3">
        <span className="text-[13px] text-text">Counts as</span>
        <GameLinkPicker games={ours} value={null} onChange={(linked) => void link(linked)} />
      </div>
      {error && <p className="mt-2 text-[12px] text-amber">{error}</p>}
    </div>
  );
}

/** Games of other PCs that count as this game, each with a way to undo it. */
function LinkedGameRows({ gameId }: { gameId: string }) {
  const { links, devices, refresh } = useLibrary();
  const [error, setError] = useState<string | null>(null);
  const linked = links.filter((link) => link.linked_game_id === gameId);
  if (linked.length === 0) return null;

  async function unlink(foreignId: string) {
    setError(null);
    try {
      await api.linkGame(foreignId, null);
      await refresh();
    } catch (linkError) {
      setError(describeError(linkError));
    }
  }

  return (
    <div className="mt-4 border-t border-rule pt-4">
      {linked.map((link) => (
        <div key={link.game_id} className="flex items-center justify-between gap-3">
          <span className="text-[13px] text-soft">
            Also played on {pcName(devices, link.origin_device_id ?? "")} as “{link.title}”
          </span>
          <Button variant="ghost" size="sm" className="text-faint" onClick={() => void unlink(link.game_id)}>
            Keep apart
          </Button>
        </div>
      ))}
      {error && <p className="mt-2 text-[12px] text-amber">{error}</p>}
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
  gameTitle,
  assets,
  onChanged,
  scannable = true,
}: {
  gameId: string;
  gameTitle: string;
  assets: GameAssetView[];
  onChanged: (assets: GameAssetView[]) => void;
  /** False for a game of another PC, whose folder is not on this PC. */
  scannable?: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cropTarget, setCropTarget] = useState<CropTarget | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const withPreview = assets.filter((asset) => asset.preview_data_url);
  const inUse = withPreview.find((asset) => asset.is_preferred);
  const added = withPreview.filter((asset) => asset.source === PLAYER_ARTWORK_SOURCE);
  const found = withPreview.filter((asset) => asset.source !== PLAYER_ARTWORK_SOURCE);
  const toDelete = withPreview.find((asset) => asset.id === deleting);

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
        filters: [{ name: "Images", extensions: ARTWORK_DIALOG_EXTENSIONS }],
      });
      if (!selected) return null;
      const source = await api.openArtworkFile(selected);
      setCropTarget({ source, label: fileName(selected), save: (crop) => api.importGameAsset(gameId, selected, crop) });
      return null;
    }, "Could not open the image");
  const adjust = (asset: GameAssetView) =>
    run(async () => {
      const source = await api.openGameAssetSource(gameId, asset.id);
      setCropTarget({
        source,
        label: source.from_original ? fileName(asset.file_path) : "The cover as it is now",
        save: (crop) => api.cropGameAsset(gameId, asset.id, crop),
      });
      return null;
    }, "Could not open the cover");
  const remove = (asset: GameAssetView) =>
    run(async () => {
      const next = await api.deleteGameAsset(gameId, asset.id);
      setDeleting(null);
      return next;
    }, "Could not delete the image");

  return (
    <AsideSection title="Cover">
      {withPreview.length === 0 ? (
        <p className="text-[13px] text-faint">
          {scannable ? "No artwork yet. Scan the game folder or add an image." : "No artwork yet. Add an image."}
        </p>
      ) : (
        <div className="flex flex-col gap-4">
          <CoverGroup
            title="Added by you"
            empty="None yet. Add an image to frame it yourself."
            noun="your image"
            assets={added}
            busy={busy}
            deleting={deleting}
            onChoose={choose}
            onDelete={setDeleting}
          />
          <CoverGroup
            title={scannable ? "Found on this PC" : "Came with its sessions"}
            empty={scannable ? "Nothing found. Scan the folder to look again." : "None came along."}
            noun="found image"
            assets={found}
            busy={busy}
            deleting={deleting}
            onChoose={choose}
            onDelete={setDeleting}
          />
        </div>
      )}
      {toDelete && (
        <DeleteCoverConfirm
          asset={toDelete}
          busy={busy}
          onDelete={() => remove(toDelete)}
          onKeep={() => setDeleting(null)}
        />
      )}
      <div className="mt-3 flex flex-wrap gap-2">
        {scannable && (
          <Button variant="outline" size="sm" onClick={rescan} disabled={busy}>
            {busy ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
            Scan folder
          </Button>
        )}
        <Button variant="outline" size="sm" onClick={addImage} disabled={busy}>
          <ImagePlus className="size-3.5" />
          Add image
        </Button>
        {inUse && (
          <Button variant="outline" size="sm" onClick={() => adjust(inUse)} disabled={busy}>
            <Crop className="size-3.5" />
            Adjust
          </Button>
        )}
      </div>
      {error && (
        <p role="alert" className="mt-2.5 text-[13px] text-amber">
          {error}
        </p>
      )}
      <CoverCropDialog
        gameTitle={gameTitle}
        target={cropTarget}
        onClose={() => setCropTarget(null)}
        onSaved={onChanged}
      />
    </AsideSection>
  );
}

/** One group of cover images, each with a button to use it and one to delete it. */
function CoverGroup({
  title,
  empty,
  noun,
  assets,
  busy,
  deleting,
  onChoose,
  onDelete,
}: {
  title: string;
  empty: string;
  /** How the buttons name an image, like "your image 2". */
  noun: string;
  assets: GameAssetView[];
  busy: boolean;
  deleting: string | null;
  onChoose: (assetId: string) => void;
  onDelete: (assetId: string) => void;
}) {
  return (
    <div>
      <div className="mb-2 flex items-baseline gap-2 text-[13px]">
        <span className="text-soft">{title}</span>
        <span className="text-faint tabular-nums">{assets.length}</span>
      </div>
      {assets.length === 0 ? (
        <p className="text-[13px] text-faint">{empty}</p>
      ) : (
        <div className="flex flex-wrap gap-2.5">
          {assets.map((asset, index) => {
            const name = `${noun} ${index + 1}`;
            const marked = asset.id === deleting;
            return (
              <div key={asset.id} className="group relative">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onChoose(asset.id)}
                  aria-pressed={asset.is_preferred}
                  aria-label={`Use ${name}${asset.is_preferred ? ", in use" : ""}`}
                  className={cn(
                    "block h-24 w-[72px] overflow-hidden rounded-[5px] border transition focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:outline-none",
                    asset.is_preferred ? "border-2 border-text" : "border-rule opacity-70 hover:opacity-100",
                    marked && "border-destructive opacity-100",
                  )}
                >
                  <img src={asset.preview_data_url ?? undefined} alt="" className="size-full object-cover" />
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onDelete(asset.id)}
                  aria-label={`Delete ${name}`}
                  className={cn(
                    "absolute -top-1.5 -right-1.5 flex size-5 items-center justify-center rounded-full border border-hairline-strong bg-surface text-soft opacity-0 transition group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:outline-none",
                    marked && "border-destructive text-destructive opacity-100",
                  )}
                >
                  <X className="size-3" />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** Asks before an image goes, in the section rather than a dialog. */
function DeleteCoverConfirm({
  asset,
  busy,
  onDelete,
  onKeep,
}: {
  asset: GameAssetView;
  busy: boolean;
  onDelete: () => void;
  onKeep: () => void;
}) {
  const keep = useRef<HTMLButtonElement>(null);
  // The safe answer has the focus, so Enter keeps the image.
  useEffect(() => keep.current?.focus(), [asset.id]);
  const found = asset.source !== PLAYER_ARTWORK_SOURCE;

  return (
    <div
      role="group"
      aria-label="Delete this image?"
      className="mt-3 border-t border-rule pt-3"
      onKeyDown={(event) => {
        if (event.key === "Escape") onKeep();
      }}
    >
      <p className="text-[13px] leading-relaxed text-soft">
        Delete this image from Vaultime? The file it came from stays where it is.
        {asset.is_preferred && " The next image becomes the cover."}
        {found && " Scan folder finds it again."}
      </p>
      <div className="mt-2.5 flex gap-2">
        <Button variant="destructive" size="sm" onClick={onDelete} disabled={busy}>
          <Trash2 className="size-3.5" />
          Delete
        </Button>
        <Button ref={keep} variant="ghost" size="sm" onClick={onKeep} disabled={busy}>
          Keep it
        </Button>
      </div>
    </div>
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

/** The last part of a path, on Windows or Linux. */
function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}
