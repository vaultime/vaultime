// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useMemo, useState } from "react";
import { Gamepad2, Loader2, Zap } from "lucide-react";
import { useGames } from "./useGames";
import { useActiveSessions } from "../sessions/useSessions";
import { AddGameDialog } from "./components/AddGameDialog";
import { DiscoverGamesDialog } from "./components/DiscoverGamesDialog";
import { EditGameDialog } from "./components/EditGameDialog";
import { DeleteGameDialog } from "./components/DeleteGameDialog";
import { GameCard } from "./components/GameCard";
import { ActivityChart } from "@/components/charts/ActivityChart";
import type { Game, GameAssetView, Session } from "@/lib/types";
import * as api from "@/lib/tauri";
import { summarizeIntegrity } from "@/lib/integrity";
import {
  buildDailyActivity,
  getSessionTotals,
  rankGamesByActiveTime,
} from "@/lib/session-stats";
import { formatCalendarDay, formatCompactDuration } from "@/lib/time";

export function LibraryPage() {
  const { games, loading, error, refresh } = useGames();
  const { activeSessions } = useActiveSessions();
  const [editingGame, setEditingGame] = useState<Game | null>(null);
  const [deletingGame, setDeletingGame] = useState<Game | null>(null);
  const [allSessions, setAllSessions] = useState<Session[]>([]);
  const [preferredAssets, setPreferredAssets] = useState<
    Record<string, GameAssetView>
  >({});

  // A string key so sessions refetch when a game starts or stops, not on every poll.
  const runningKey = [...new Set(activeSessions.map((s) => s.game_id))]
    .sort()
    .join(",");
  const runningGameIds = useMemo(
    () => new Set(runningKey ? runningKey.split(",") : []),
    [runningKey],
  );

  useEffect(() => {
    api
      .listSessions()
      .then(setAllSessions)
      .catch(() => {});
  }, [games, runningKey]);

  useEffect(() => {
    api
      .listPreferredGameAssets()
      .then((assets) => {
        const nextAssets: Record<string, GameAssetView> = {};
        for (const asset of assets) {
          nextAssets[asset.game_id] = asset;
        }
        setPreferredAssets(nextAssets);
      })
      .catch(() => {});
  }, [games]);

  const totalsByGame = useMemo(() => {
    const map: Record<
      string,
      {
        runtimeMs: number;
        activeMs: number;
        idleMs: number;
        sessionsCount: number;
        lastPlayedAt: string | null;
        integrityStatus: string;
        suspiciousCount: number;
        recoveredCount: number;
        sessions: Session[];
      }
    > = {};
    for (const s of allSessions) {
      const totals = map[s.game_id] ?? {
        runtimeMs: 0,
        activeMs: 0,
        idleMs: 0,
        sessionsCount: 0,
        lastPlayedAt: null,
        integrityStatus: "local",
        suspiciousCount: 0,
        recoveredCount: 0,
        sessions: [],
      };
      totals.runtimeMs += s.runtime_ms;
      totals.activeMs += s.active_ms;
      totals.idleMs += s.idle_ms;
      totals.sessionsCount += 1;
      totals.sessions.push(s);
      if (!totals.lastPlayedAt || s.started_at_wall > totals.lastPlayedAt) {
        totals.lastPlayedAt = s.started_at_wall;
      }
      map[s.game_id] = totals;
    }

    for (const totals of Object.values(map)) {
      const integrity = summarizeIntegrity(totals.sessions);
      totals.integrityStatus = integrity.overallStatus;
      totals.suspiciousCount = integrity.suspiciousCount;
      totals.recoveredCount = integrity.recoveredCount;
    }

    return map;
  }, [allSessions]);

  const libraryTotals = useMemo(
    () => getSessionTotals(allSessions),
    [allSessions],
  );
  const recentActivity = useMemo(
    () => buildDailyActivity(allSessions, 14),
    [allSessions],
  );
  const topGames = useMemo(
    () => rankGamesByActiveTime(games, allSessions, 5),
    [games, allSessions],
  );
  const activeDays = useMemo(
    () => recentActivity.filter((point) => point.activeMs > 0).length,
    [recentActivity],
  );

  return (
    <div className="space-y-8">
      <div className="flex items-end justify-between">
        <div>
          <h1 className="text-3xl font-extrabold tracking-tight">Library</h1>
          <p className="mt-1 text-sm text-muted-foreground">
            {games.length > 0
              ? `${games.length} game${games.length === 1 ? "" : "s"} tracked`
              : "Your game collection and playtime at a glance."}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <DiscoverGamesDialog onImported={refresh} />
          <AddGameDialog onAdded={refresh} />
        </div>
      </div>

      {!loading && !error && games.length > 0 && (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.3fr)_minmax(300px,0.7fr)]">
          <div className="relative overflow-hidden rounded-3xl border border-border/70 bg-card/60 p-6">
            <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_at_top_left,rgba(135,88,255,0.2),transparent_50%),radial-gradient(ellipse_at_bottom_right,rgba(59,210,180,0.1),transparent_50%)]" />

            <div className="relative space-y-6">
              <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
                <div className="rounded-2xl border border-[color:var(--color-chart-1)]/20 bg-[color:var(--color-chart-1)]/[0.06] p-4">
                  <p className="text-[10px] uppercase tracking-[0.22em] text-[color:var(--color-chart-1)]">
                    Active Time
                  </p>
                  <p className="mt-2 text-2xl font-bold tabular-nums">
                    {formatCompactDuration(libraryTotals.activeMs)}
                  </p>
                </div>
                <div className="rounded-2xl border border-[color:var(--color-chart-2)]/20 bg-[color:var(--color-chart-2)]/[0.06] p-4">
                  <p className="text-[10px] uppercase tracking-[0.22em] text-[color:var(--color-chart-2)]">
                    Runtime
                  </p>
                  <p className="mt-2 text-2xl font-bold tabular-nums">
                    {formatCompactDuration(libraryTotals.runtimeMs)}
                  </p>
                </div>
                <div className="rounded-2xl border border-[color:var(--color-chart-4)]/20 bg-[color:var(--color-chart-4)]/[0.06] p-4">
                  <p className="text-[10px] uppercase tracking-[0.22em] text-[color:var(--color-chart-4)]">
                    Sessions
                  </p>
                  <p className="mt-2 text-2xl font-bold tabular-nums">
                    {libraryTotals.sessionsCount}
                  </p>
                </div>
                <div className="rounded-2xl border border-[color:var(--color-chart-3)]/20 bg-[color:var(--color-chart-3)]/[0.06] p-4">
                  <p className="text-[10px] uppercase tracking-[0.22em] text-[color:var(--color-chart-3)]">
                    Active Days
                  </p>
                  <p className="mt-2 text-2xl font-bold tabular-nums">
                    {activeDays}
                    <span className="text-sm font-normal text-muted-foreground">
                      /14
                    </span>
                  </p>
                </div>
              </div>

              <ActivityChart points={recentActivity} compact />
            </div>
          </div>

          <div className="rounded-3xl border border-border/70 bg-card/60 p-6">
            <div className="mb-5 flex items-center gap-2">
              <Zap className="h-4 w-4 text-[color:var(--color-chart-1)]" />
              <h2 className="text-sm font-bold uppercase tracking-[0.18em]">
                Top Titles
              </h2>
            </div>

            {topGames.length === 0 ? (
              <div className="rounded-2xl border border-dashed border-white/10 bg-white/[0.03] p-4 text-sm text-muted-foreground">
                Play a tracked game to see rankings.
              </div>
            ) : (
              <div className="space-y-2">
                {topGames.map((entry, index) => (
                  <div
                    key={entry.game.id}
                    className="group/rank flex items-center gap-3 rounded-2xl border border-border/70 bg-white/[0.03] px-4 py-3 transition-colors hover:bg-white/[0.06]"
                  >
                    <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-[color:var(--color-chart-1)]/15 text-xs font-bold text-[color:var(--color-chart-1)]">
                      {index + 1}
                    </span>
                    <div className="min-w-0 flex-1">
                      <p className="truncate text-sm font-medium">
                        {entry.game.title}
                      </p>
                      <p className="truncate text-[11px] text-muted-foreground">
                        {entry.lastPlayedAt
                          ? `Last ${formatCalendarDay(entry.lastPlayedAt)}`
                          : "No sessions yet"}
                      </p>
                    </div>
                    <div className="text-right">
                      <p className="text-sm font-bold tabular-nums">
                        {formatCompactDuration(entry.activeMs)}
                      </p>
                      <p className="text-[11px] tabular-nums text-muted-foreground">
                        {entry.sessionsCount} session
                        {entry.sessionsCount === 1 ? "" : "s"}
                      </p>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      )}

      {loading && (
        <div className="flex h-64 items-center justify-center">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      )}

      {error && (
        <div className="rounded-2xl border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
          {error}
        </div>
      )}

      {!loading && !error && games.length === 0 && (
        <div className="flex h-72 flex-col items-center justify-center rounded-3xl border border-dashed border-white/10 bg-white/[0.02]">
          <div className="rounded-2xl bg-[color:var(--color-chart-1)]/10 p-4">
            <Gamepad2 className="h-10 w-10 text-[color:var(--color-chart-1)]/60" />
          </div>
          <p className="mt-4 text-sm font-medium text-foreground">
            Your vault is empty
          </p>
          <p className="mt-1 text-xs text-muted-foreground">
            Add a game manually or discover your Steam library to get started.
          </p>
          <div className="mt-5 flex items-center gap-2">
            <DiscoverGamesDialog onImported={refresh} />
            <AddGameDialog onAdded={refresh} />
          </div>
        </div>
      )}

      {!loading && games.length > 0 && (
        <div className="stagger-grid grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {games.map((game) => (
            <GameCard
              key={game.id}
              game={game}
              detailTo={`/library/${game.id}`}
              isRunning={runningGameIds.has(game.id)}
              totalRuntimeMs={totalsByGame[game.id]?.runtimeMs ?? 0}
              totalActiveMs={totalsByGame[game.id]?.activeMs ?? 0}
              sessionCount={totalsByGame[game.id]?.sessionsCount ?? 0}
              lastPlayedAt={totalsByGame[game.id]?.lastPlayedAt ?? null}
              coverImageUrl={
                preferredAssets[game.id]?.preview_data_url ?? null
              }
              integrityStatus={totalsByGame[game.id]?.integrityStatus ?? "local"}
              suspiciousCount={totalsByGame[game.id]?.suspiciousCount ?? 0}
              recoveredCount={totalsByGame[game.id]?.recoveredCount ?? 0}
              onEdit={setEditingGame}
              onDelete={setDeletingGame}
            />
          ))}
        </div>
      )}

      <EditGameDialog
        game={editingGame}
        onClose={() => setEditingGame(null)}
        onSaved={refresh}
      />
      <DeleteGameDialog
        game={deletingGame}
        onClose={() => setDeletingGame(null)}
        onDeleted={refresh}
      />
    </div>
  );
}
