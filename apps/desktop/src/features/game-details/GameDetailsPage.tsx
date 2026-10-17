// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useState } from "react";
import { Link, Navigate, useNavigate, useParams } from "react-router";
import { Eye, EyeOff, Pencil, Plus, Trash2 } from "lucide-react";
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
import { useLibrary, usePlaying } from "@/features/library/library-context";
import { AddSessionDialog } from "@/features/game-details/AddSessionDialog";
import { GameHistory } from "@/features/game-details/GameHistory";
import { StatusPicker } from "@/features/game-details/StatusPicker";
import { SessionLine } from "@/features/sessions/components/SessionLine";
import { FOLDER_MATCH_LAUNCHERS, GAME_RECENT_SESSIONS, STEAM_LAUNCHER } from "@/lib/constants";
import { launchersOf, linkedTo, whereFrom } from "@/lib/links";
import { gamePlaytime } from "@/lib/sentences";
import { countsAsPlay } from "@/lib/sessions";
import { stepsAside } from "@/lib/steps-aside";
import * as api from "@/lib/tauri";
import { formatCalendarDay, formatHoursMinutes } from "@/lib/time";
import type { Game, GameAssetView, SessionEvent } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { capitalize } from "@/lib/words";
import { AsideSection, EarlierSection, TotalRow } from "./Aside";
import { CoverPicker } from "./CoverPicker";
import { EventLog } from "./EventLog";
import { ForeignGameRows, LinkedGameRows, SameGameRow, StepsAsideRow } from "./TrackingRows";

export function GameDetailsPage() {
  const { gameId } = useParams();
  // A fresh page per game, so nothing from the last game lingers while loading.
  return gameId ? <GamePage key={gameId} gameId={gameId} /> : null;
}

function GamePage({ gameId }: { gameId: string }) {
  const navigate = useNavigate();
  const { summaries, sessions: allSessions, loaded, refresh, notes, saveNote, setStatus, links, thisDeviceId } =
    useLibrary();
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
  const playing = usePlaying().has(gameId);

  // Events explain flagged sessions. Reload them whenever the sessions change.
  useEffect(() => {
    let cancelled = false;
    api
      .getNotableEventsForGame(gameId)
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

  // A game that counts as another one has its page there.
  const countsAs = links.find((link) => link.game_id === gameId)?.linked_game_id;
  if (!summary && countsAs) return <Navigate to={`/library/${countsAs}`} replace />;

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
  const idleMs = sessions.reduce((sum, session) => sum + session.counted_idle_ms, 0);

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
              <span>{launchersOf(game, links, thisDeviceId) ?? capitalize(whereFrom(game.launcher_source))}</span>
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
            <LinkedGameRows gameId={game.id} />
            {!game.origin_device_id && (
              <SameGameRow game={game} onLinked={(targetId) => navigate(`/library/${targetId}`)} />
            )}
            <StepsAsideRow
              game={game}
              on={stepsAside(game)}
              sessions={allSessions}
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
          steamGame={
            game.launcher_source === STEAM_LAUNCHER ||
            summary.earlier?.source === STEAM_LAUNCHER ||
            linkedTo(game.id, links, thisDeviceId).installs.some((link) => link.launcher_source === STEAM_LAUNCHER)
          }
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
