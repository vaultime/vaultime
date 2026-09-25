// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Activity, Clock, Loader2, Shield, TimerReset } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { Game } from "@/lib/types";
import * as api from "@/lib/tauri";
import { useSessions, useActiveSessions } from "./useSessions";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { ActivityChart } from "@/components/charts/ActivityChart";
import { IntegrityBadge } from "@/components/status/IntegrityBadge";
import { summarizeIntegrity } from "@/lib/integrity";
import {
  buildDailyActivity,
  getSessionTotals,
  rankGamesByActiveTime,
} from "@/lib/session-stats";
import { formatCompactDuration, formatDuration } from "@/lib/time";
import { SessionTimeline } from "./components/SessionTimeline";

export function SessionsPage() {
  const { sessions, loading, error, refresh } = useSessions();
  const { activeSessions } = useActiveSessions();
  const [gameMap, setGameMap] = useState<Record<string, string>>({});
  const [games, setGames] = useState<Game[]>([]);
  const summary = useMemo(() => {
    return getSessionTotals(sessions);
  }, [sessions]);
  const dailyActivity = useMemo(
    () => buildDailyActivity(sessions, 14),
    [sessions],
  );
  const integritySummary = useMemo(
    () => summarizeIntegrity(sessions),
    [sessions],
  );
  const topGames = useMemo(
    () => rankGamesByActiveTime(games, sessions, 5),
    [games, sessions],
  );

  // Build a game name lookup from the game list.
  useEffect(() => {
    api.listGames().then((games: Game[]) => {
      const map: Record<string, string> = {};
      for (const g of games) {
        map[g.id] = g.title;
      }
      setGames(games);
      setGameMap(map);
    });
  }, [sessions]);

  // Auto-refresh when active sessions change.
  useEffect(() => {
    if (activeSessions.length > 0) {
      refresh();
    }
  }, [activeSessions, refresh]);

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4">
        <p className="text-sm text-destructive">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Sessions</h1>
        <p className="text-muted-foreground">
          Timeline, pacing, and totals across your tracked play history.
        </p>
      </div>

      <div className="grid gap-4 md:grid-cols-4">
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle>Runtime</CardTitle>
          </CardHeader>
          <CardContent className="pt-0 text-2xl font-semibold tabular-nums">
            {formatDuration(summary.runtimeMs)}
          </CardContent>
        </Card>
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle>Active</CardTitle>
          </CardHeader>
          <CardContent className="pt-0 text-2xl font-semibold tabular-nums">
            {formatDuration(summary.activeMs)}
          </CardContent>
        </Card>
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle>Idle</CardTitle>
          </CardHeader>
          <CardContent className="pt-0 text-2xl font-semibold tabular-nums">
            {formatDuration(summary.idleMs)}
          </CardContent>
        </Card>
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle>Live Sessions</CardTitle>
          </CardHeader>
          <CardContent className="pt-0 text-2xl font-semibold tabular-nums">
            {activeSessions.length}
          </CardContent>
        </Card>
      </div>

      <Card className="relative overflow-hidden border border-border/70 bg-card/70">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(147,51,234,0.16),transparent_34%),radial-gradient(circle_at_bottom_right,rgba(59,130,246,0.12),transparent_36%)]" />
        <CardHeader className="relative">
          <CardTitle className="flex items-center gap-2">
            <Shield className="h-4 w-4 text-primary" />
            Integrity Overview
          </CardTitle>
          <CardDescription>{integritySummary.note}</CardDescription>
        </CardHeader>
        <CardContent className="relative flex flex-wrap items-center gap-3">
          <IntegrityBadge status={integritySummary.overallStatus} />
          <div className="rounded-full border border-border/70 bg-background/45 px-3 py-1 text-xs text-muted-foreground">
            {integritySummary.localCount} local
          </div>
          <div className="rounded-full border border-border/70 bg-background/45 px-3 py-1 text-xs text-muted-foreground">
            {integritySummary.recoveredCount} recovered
          </div>
          <div className="rounded-full border border-border/70 bg-background/45 px-3 py-1 text-xs text-muted-foreground">
            {integritySummary.suspiciousCount} flagged
          </div>
        </CardContent>
      </Card>

      {sessions.length > 0 && (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.25fr)_minmax(320px,0.75fr)]">
          <Card className="border border-border/70">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <Activity className="h-4 w-4 text-primary" />
                Recent Activity
              </CardTitle>
              <CardDescription>
                Last 14 days of runtime with active play highlighted inside each bar.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-4">
              <ActivityChart points={dailyActivity} />
              <div className="flex flex-wrap items-center gap-4 text-xs text-muted-foreground">
                <div className="flex items-center gap-2">
                  <span className="h-2.5 w-2.5 rounded-full bg-[color:var(--color-chart-1)]" />
                  Active time
                </div>
                <div className="flex items-center gap-2">
                  <span className="h-2.5 w-2.5 rounded-full bg-[color:var(--color-chart-3)]/60" />
                  Runtime envelope
                </div>
              </div>
            </CardContent>
          </Card>

          <Card className="border border-border/70">
            <CardHeader>
              <CardTitle className="flex items-center gap-2">
                <TimerReset className="h-4 w-4 text-primary" />
                Most Played
              </CardTitle>
              <CardDescription>
                Games with the highest active time in your current history.
              </CardDescription>
            </CardHeader>
            <CardContent className="space-y-3">
              {topGames.map((entry, index) => (
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
                      {entry.sessionsCount} session
                      {entry.sessionsCount === 1 ? "" : "s"}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-sm font-semibold">
                      {formatCompactDuration(entry.activeMs)}
                    </p>
                    <p className="text-xs text-muted-foreground">
                      Runtime {formatCompactDuration(entry.runtimeMs)}
                    </p>
                  </div>
                </div>
              ))}
            </CardContent>
          </Card>
        </div>
      )}

      {sessions.length === 0 ? (
        <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed border-border">
          <Clock className="mb-3 h-10 w-10 text-muted-foreground/50" />
          <p className="text-sm text-muted-foreground">
            No sessions recorded yet. Start playing a tracked game.
          </p>
        </div>
      ) : (
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle>Timeline</CardTitle>
            <CardDescription>
              Grouped by day so long play stretches and repeat launches are easy to read.
            </CardDescription>
          </CardHeader>
          <CardContent>
            <SessionTimeline
              sessions={sessions}
              gameMap={gameMap}
              emptyMessage="No sessions recorded yet. Start playing a tracked game."
            />
          </CardContent>
        </Card>
      )}
    </div>
  );
}
