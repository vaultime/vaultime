// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Activity, Clock, Loader2, Shield, TimerReset, Zap } from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { ACTIVITY_CHART_DAYS } from "@/lib/constants";
import type { Game } from "@/lib/types";
import * as api from "@/lib/tauri";
import { useSessions, useActiveSessions } from "./useSessions";
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
    () => buildDailyActivity(sessions, ACTIVITY_CHART_DAYS),
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

  useEffect(() => {
    api
      .listGames()
      .then((games: Game[]) => {
        const map: Record<string, string> = {};
        for (const g of games) {
          map[g.id] = g.title;
        }
        setGames(games);
        setGameMap(map);
      })
      .catch(() => {});
  }, []);

  // Refresh on every change to the open sessions, including the last one ending,
  // so a finished session stops showing as live. useSessions covers the first load.
  const seenActiveRef = useRef(activeSessions);
  useEffect(() => {
    if (seenActiveRef.current === activeSessions) {
      return;
    }
    seenActiveRef.current = activeSessions;
    void refresh();
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
      <div className="rounded-2xl border border-destructive/50 bg-destructive/10 p-4">
        <p className="text-sm text-destructive">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-8">
      <div>
        <h1 className="text-3xl font-extrabold tracking-tight">Sessions</h1>
        <p className="mt-1 text-sm text-muted-foreground">
          Timeline, pacing, and totals across your tracked play history.
        </p>
      </div>

      <div className="grid gap-3 sm:grid-cols-2 md:grid-cols-4">
        <div className="rounded-2xl border border-[color:var(--color-chart-1)]/20 bg-[color:var(--color-chart-1)]/[0.06] p-4">
          <div className="flex items-center gap-2">
            <Zap className="h-3.5 w-3.5 text-[color:var(--color-chart-1)]" />
            <p className="text-[10px] uppercase tracking-[0.22em] text-[color:var(--color-chart-1)]">
              Active
            </p>
          </div>
          <p className="mt-2 text-2xl font-bold tabular-nums">
            {formatDuration(summary.activeMs)}
          </p>
        </div>
        <div className="rounded-2xl border border-[color:var(--color-chart-2)]/20 bg-[color:var(--color-chart-2)]/[0.06] p-4">
          <div className="flex items-center gap-2">
            <TimerReset className="h-3.5 w-3.5 text-[color:var(--color-chart-2)]" />
            <p className="text-[10px] uppercase tracking-[0.22em] text-[color:var(--color-chart-2)]">
              Runtime
            </p>
          </div>
          <p className="mt-2 text-2xl font-bold tabular-nums">
            {formatDuration(summary.runtimeMs)}
          </p>
        </div>
        <div className="rounded-2xl border border-[color:var(--color-chart-4)]/20 bg-[color:var(--color-chart-4)]/[0.06] p-4">
          <div className="flex items-center gap-2">
            <Clock className="h-3.5 w-3.5 text-[color:var(--color-chart-4)]" />
            <p className="text-[10px] uppercase tracking-[0.22em] text-[color:var(--color-chart-4)]">
              Idle
            </p>
          </div>
          <p className="mt-2 text-2xl font-bold tabular-nums">
            {formatDuration(summary.idleMs)}
          </p>
        </div>
        <div className={`rounded-2xl border p-4 ${
          activeSessions.length > 0
            ? "border-green-500/30 bg-green-500/[0.06]"
            : "border-border/70 bg-white/[0.03]"
        }`}>
          <div className="flex items-center gap-2">
            {activeSessions.length > 0 ? (
              <span className="relative flex h-2.5 w-2.5">
                <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-green-400 opacity-75" />
                <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-green-400" />
              </span>
            ) : (
              <Activity className="h-3.5 w-3.5 text-muted-foreground" />
            )}
            <p className={`text-[10px] uppercase tracking-[0.22em] ${
              activeSessions.length > 0 ? "text-green-400" : "text-muted-foreground"
            }`}>
              Live
            </p>
          </div>
          <p className="mt-2 text-2xl font-bold tabular-nums">
            {activeSessions.length}
          </p>
        </div>
      </div>

      <div className="relative overflow-hidden rounded-3xl border border-border/70 bg-card/60 p-6">
        <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(ellipse_at_top_left,rgba(147,51,234,0.12),transparent_45%),radial-gradient(ellipse_at_bottom_right,rgba(59,130,246,0.08),transparent_45%)]" />
        <div className="relative">
          <div className="mb-4 flex items-center gap-2">
            <Shield className="h-4 w-4 text-primary" />
            <h2 className="text-sm font-bold uppercase tracking-[0.18em]">
              Integrity
            </h2>
          </div>
          <p className="mb-4 text-xs text-muted-foreground">
            {integritySummary.note}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <IntegrityBadge status={integritySummary.overallStatus} />
            <span className="rounded-full border border-border/70 bg-white/[0.04] px-2.5 py-1 text-[11px] tabular-nums text-muted-foreground">
              {integritySummary.localCount} local
            </span>
            <span className="rounded-full border border-border/70 bg-white/[0.04] px-2.5 py-1 text-[11px] tabular-nums text-muted-foreground">
              {integritySummary.recoveredCount} recovered
            </span>
            {integritySummary.suspiciousCount > 0 && (
              <span className="rounded-full border border-amber-500/20 bg-amber-500/[0.06] px-2.5 py-1 text-[11px] tabular-nums text-amber-400">
                {integritySummary.suspiciousCount} flagged
              </span>
            )}
          </div>
        </div>
      </div>

      {sessions.length > 0 && (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1.3fr)_minmax(300px,0.7fr)]">
          <div className="rounded-3xl border border-border/70 bg-card/60 p-6">
            <div className="mb-5 flex items-center gap-2">
              <Activity className="h-4 w-4 text-[color:var(--color-chart-1)]" />
              <h2 className="text-sm font-bold uppercase tracking-[0.18em]">
                14-Day Activity
              </h2>
            </div>
            <ActivityChart points={dailyActivity} />
            <div className="mt-4 flex flex-wrap items-center gap-4 text-[11px] text-muted-foreground">
              <div className="flex items-center gap-2">
                <span className="h-2.5 w-2.5 rounded-full bg-[color:var(--color-chart-1)]" />
                Active
              </div>
              <div className="flex items-center gap-2">
                <span className="h-2.5 w-2.5 rounded-full bg-[color:var(--color-chart-3)]/60" />
                Runtime
              </div>
            </div>
          </div>

          <div className="rounded-3xl border border-border/70 bg-card/60 p-6">
            <div className="mb-5 flex items-center gap-2">
              <Zap className="h-4 w-4 text-[color:var(--color-chart-1)]" />
              <h2 className="text-sm font-bold uppercase tracking-[0.18em]">
                Most Played
              </h2>
            </div>
            <div className="space-y-2">
              {topGames.map((entry, index) => (
                <div
                  key={entry.game.id}
                  className="flex items-center gap-3 rounded-2xl border border-border/70 bg-white/[0.03] px-4 py-3 transition-colors hover:bg-white/[0.06]"
                >
                  <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-[color:var(--color-chart-1)]/15 text-xs font-bold text-[color:var(--color-chart-1)]">
                    {index + 1}
                  </span>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium">
                      {entry.game.title}
                    </p>
                    <p className="truncate text-[11px] text-muted-foreground">
                      {entry.sessionsCount} session
                      {entry.sessionsCount === 1 ? "" : "s"}
                    </p>
                  </div>
                  <div className="text-right">
                    <p className="text-sm font-bold tabular-nums">
                      {formatCompactDuration(entry.activeMs)}
                    </p>
                    <p className="text-[11px] tabular-nums text-muted-foreground">
                      {formatCompactDuration(entry.runtimeMs)} total
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {sessions.length === 0 ? (
        <div className="flex h-64 flex-col items-center justify-center rounded-3xl border border-dashed border-white/10 bg-white/[0.02]">
          <div className="rounded-2xl bg-white/[0.04] p-4">
            <Clock className="h-10 w-10 text-muted-foreground/40" />
          </div>
          <p className="mt-4 text-sm text-muted-foreground">
            No sessions recorded yet. Start playing a tracked game.
          </p>
        </div>
      ) : (
        <div className="rounded-3xl border border-border/70 bg-card/60 p-6">
          <div className="mb-5">
            <h2 className="text-sm font-bold uppercase tracking-[0.18em]">
              Timeline
            </h2>
            <p className="mt-1 text-xs text-muted-foreground">
              Grouped by day for easy scanning.
            </p>
          </div>
          <SessionTimeline
            sessions={sessions}
            gameMap={gameMap}
            emptyMessage="No sessions recorded yet. Start playing a tracked game."
          />
        </div>
      )}
    </div>
  );
}
