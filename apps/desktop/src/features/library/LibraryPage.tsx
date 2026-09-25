// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useMemo, useState } from "react";
import { Flame, Gamepad2, Loader2, TimerReset } from "lucide-react";
import { useGames } from "./useGames";
import { useActiveSessions } from "../sessions/useSessions";
import { AddGameDialog } from "./components/AddGameDialog";
import { EditGameDialog } from "./components/EditGameDialog";
import { DeleteGameDialog } from "./components/DeleteGameDialog";
import { GameCard } from "./components/GameCard";
import { ActivityChart } from "@/components/charts/ActivityChart";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { Game, Session } from "@/lib/types";
import * as api from "@/lib/tauri";
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

  // Fetch all sessions for playtime totals.
  useEffect(() => {
    api.listSessions().then(setAllSessions).catch(() => {});
  }, [games, activeSessions]);

  // Build a set of currently running game IDs.
  const runningGameIds = useMemo(() => {
    const ids = new Set<string>();
    for (const s of activeSessions) {
      ids.add(s.game_id);
    }
    return ids;
  }, [activeSessions]);

  // Build total timing stats per game.
  const totalsByGame = useMemo(() => {
    const map: Record<
      string,
      {
        runtimeMs: number;
        activeMs: number;
        idleMs: number;
        sessionsCount: number;
        lastPlayedAt: string | null;
      }
    > = {};
    for (const s of allSessions) {
      const totals = map[s.game_id] ?? {
        runtimeMs: 0,
        activeMs: 0,
        idleMs: 0,
        sessionsCount: 0,
        lastPlayedAt: null,
      };
      totals.runtimeMs += s.runtime_ms;
      totals.activeMs += s.active_ms;
      totals.idleMs += s.idle_ms;
      totals.sessionsCount += 1;
      if (!totals.lastPlayedAt || s.started_at_wall > totals.lastPlayedAt) {
        totals.lastPlayedAt = s.started_at_wall;
      }
      map[s.game_id] = totals;
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
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Library</h1>
          <p className="text-muted-foreground">
            {games.length > 0
              ? `${games.length} game${games.length === 1 ? "" : "s"} tracked`
              : "Your game collection and playtime at a glance."}
          </p>
        </div>
        <AddGameDialog onAdded={refresh} />
      </div>

      {!loading && !error && games.length > 0 && (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(320px,0.8fr)]">
          <Card className="relative overflow-hidden border border-border/70 bg-card/70">
            <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(119,91,255,0.2),transparent_38%),radial-gradient(circle_at_bottom_right,rgba(61,179,160,0.14),transparent_35%)]" />
            <CardHeader className="relative">
              <CardTitle className="flex items-center gap-2">
                <Flame className="h-4 w-4 text-primary" />
                Library Snapshot
              </CardTitle>
              <CardDescription>
                Your recent pace and the games carrying the most active time.
              </CardDescription>
            </CardHeader>
            <CardContent className="relative space-y-6">
              <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
                <div className="rounded-2xl border border-border/70 bg-background/55 p-4">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Active Time
                  </p>
                  <p className="mt-2 text-2xl font-semibold">
                    {formatCompactDuration(libraryTotals.activeMs)}
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-background/55 p-4">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Runtime
                  </p>
                  <p className="mt-2 text-2xl font-semibold">
                    {formatCompactDuration(libraryTotals.runtimeMs)}
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-background/55 p-4">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Sessions
                  </p>
                  <p className="mt-2 text-2xl font-semibold">
                    {libraryTotals.sessionsCount}
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-background/55 p-4">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Active Days
                  </p>
                  <p className="mt-2 text-2xl font-semibold">{activeDays}/14</p>
                </div>
              </div>

              <ActivityChart points={recentActivity} compact />
            </CardContent>
          </Card>

          <Card className="border border-border/70">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <TimerReset className="h-4 w-4 text-primary" />
                Top Titles
              </CardTitle>
              <CardDescription>
                Highest active playtime across your tracked library.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {topGames.length === 0 ? (
                <div className="rounded-2xl border border-dashed border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                  Start a tracked session to populate library rankings.
                </div>
              ) : (
                topGames.map((entry, index) => (
                  <div
                    key={entry.game.id}
                    className="flex items-center justify-between rounded-2xl border border-border/70 bg-muted/20 px-4 py-3"
                  >
                    <div className="min-w-0">
                      <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                        #{index + 1}
                      </p>
                      <p className="truncate text-sm font-medium">
                        {entry.game.title}
                      </p>
                      <p className="truncate text-xs text-muted-foreground">
                        {entry.lastPlayedAt
                          ? `Last played ${formatCalendarDay(entry.lastPlayedAt)}`
                          : "No launch history yet"}
                      </p>
                    </div>
                    <div className="text-right">
                      <p className="text-sm font-semibold">
                        {formatCompactDuration(entry.activeMs)}
                      </p>
                      <p className="text-xs text-muted-foreground">
                        {entry.sessionsCount} session
                        {entry.sessionsCount === 1 ? "" : "s"}
                      </p>
                    </div>
                  </div>
                ))
              )}
            </CardContent>
          </Card>
        </div>
      )}

      {loading && (
        <div className="flex h-64 items-center justify-center">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
          {error}
        </div>
      )}

      {!loading && !error && games.length === 0 && (
        <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed border-border">
          <Gamepad2 className="mb-3 h-10 w-10 text-muted-foreground/50" />
          <p className="text-sm text-muted-foreground">
            No games added yet. Click &quot;Add Game&quot; to get started.
          </p>
        </div>
      )}

      {!loading && games.length > 0 && (
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
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
