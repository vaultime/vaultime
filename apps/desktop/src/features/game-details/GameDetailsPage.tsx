// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { ArrowLeft, Clock3, Gamepad2, Loader2, PlayCircle, Timer } from "lucide-react";
import { Link, useParams } from "react-router";
import { useEffect, useMemo, useState } from "react";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { ActivityChart } from "@/components/charts/ActivityChart";
import { SessionTimeline } from "@/features/sessions/components/SessionTimeline";
import {
  buildDailyActivity,
  getSessionTotals,
} from "@/lib/session-stats";
import {
  formatCalendarDay,
  formatCompactDuration,
  formatLongDate,
} from "@/lib/time";
import type { Game, Session } from "@/lib/types";
import * as api from "@/lib/tauri";
import { useActiveSessions } from "@/features/sessions";

function DetailStatCard({
  title,
  value,
  hint,
}: {
  title: string;
  value: string;
  hint: string;
}) {
  return (
    <Card className="border border-border/70 bg-card/70">
      <CardHeader className="pb-2">
        <CardDescription>{title}</CardDescription>
      </CardHeader>
      <CardContent>
        <p className="text-2xl font-semibold tracking-tight">{value}</p>
        <p className="mt-2 text-xs text-muted-foreground">{hint}</p>
      </CardContent>
    </Card>
  );
}

export function GameDetailsPage() {
  const { gameId } = useParams();
  const { activeSessions } = useActiveSessions();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [game, setGame] = useState<Game | null>(null);
  const [sessions, setSessions] = useState<Session[]>([]);

  useEffect(() => {
    if (!gameId) {
      setError("Game ID is missing.");
      setLoading(false);
      return;
    }

    const currentGameId = gameId;

    let cancelled = false;

    async function load() {
      try {
        setLoading(true);
        setError(null);
        const [fetchedGame, fetchedSessions] = await Promise.all([
          api.getGame(currentGameId),
          api.getSessionsForGame(currentGameId),
        ]);

        if (cancelled) {
          return;
        }

        setGame(fetchedGame);
        setSessions(fetchedSessions);
      } catch (loadError) {
        if (!cancelled) {
          setError(String(loadError));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();

    return () => {
      cancelled = true;
    };
  }, [gameId]);

  useEffect(() => {
    if (!gameId) {
      return;
    }

    if (!activeSessions.some((session) => session.game_id === gameId)) {
      return;
    }

    api.getSessionsForGame(gameId).then(setSessions).catch(() => {});
  }, [activeSessions, gameId]);

  const totals = useMemo(() => getSessionTotals(sessions), [sessions]);
  const dailyActivity = useMemo(
    () => buildDailyActivity(sessions, 14),
    [sessions],
  );
  const peakDay = useMemo(() => {
    return [...dailyActivity].sort((a, b) => b.activeMs - a.activeMs)[0] ?? null;
  }, [dailyActivity]);
  const runningNow = activeSessions.some((session) => session.game_id === gameId);

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error || !game) {
    return (
      <div className="space-y-4">
        <Link
          to="/library"
          className="inline-flex h-8 items-center gap-1.5 rounded-lg border border-border bg-background px-2.5 text-sm font-medium transition-colors hover:bg-muted"
        >
          <ArrowLeft className="h-4 w-4" />
          Back To Library
        </Link>
        <div className="rounded-2xl border border-destructive/50 bg-destructive/10 p-5 text-sm text-destructive">
          {error ?? "Game not found."}
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <Link
        to="/library"
        className="inline-flex h-8 items-center gap-1.5 rounded-lg px-2.5 text-sm font-medium transition-colors hover:bg-muted"
      >
        <ArrowLeft className="h-4 w-4" />
        Back To Library
      </Link>

      <section className="relative overflow-hidden rounded-[2rem] border border-border/70 bg-card/70">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(119,91,255,0.24),transparent_38%),radial-gradient(circle_at_bottom_right,rgba(66,191,165,0.16),transparent_36%)]" />
        <div className="relative grid gap-6 p-6 lg:grid-cols-[220px_minmax(0,1fr)] lg:p-8">
          <div className="flex h-56 items-center justify-center rounded-[1.75rem] border border-white/10 bg-gradient-to-br from-primary/18 via-primary/8 to-transparent">
            <Gamepad2 className="h-20 w-20 text-primary/70" />
          </div>

          <div className="space-y-5">
            <div className="flex flex-wrap items-start justify-between gap-4">
              <div className="space-y-2">
                <div className="flex flex-wrap items-center gap-2">
                  <h1 className="text-3xl font-semibold tracking-tight">
                    {game.title}
                  </h1>
                  {runningNow && (
                    <Badge className="bg-green-600 text-white">Running</Badge>
                  )}
                </div>
                <p className="max-w-2xl text-sm leading-6 text-muted-foreground">
                  {game.executable_path ??
                    game.install_folder ??
                    "Manual game entry without an executable path yet."}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-background/55 px-4 py-3 text-right">
                <p className="text-xs uppercase tracking-[0.25em] text-muted-foreground">
                  Last played
                </p>
                <p className="mt-2 text-sm font-medium">
                  {totals.lastPlayedAt
                    ? formatLongDate(totals.lastPlayedAt)
                    : "No sessions yet"}
                </p>
              </div>
            </div>

            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
              <DetailStatCard
                title="Active Time"
                value={formatCompactDuration(totals.activeMs)}
                hint={`${Math.round(totals.activeRatio * 100)}% of tracked runtime counted as active play`}
              />
              <DetailStatCard
                title="Runtime"
                value={formatCompactDuration(totals.runtimeMs)}
                hint={`${formatCompactDuration(totals.idleMs)} spent in idle or background time`}
              />
              <DetailStatCard
                title="Sessions"
                value={String(totals.sessionsCount)}
                hint={
                  totals.sessionsCount > 0
                    ? `${formatCompactDuration(totals.averageActiveMs)} average active time per session`
                    : "Start a tracked session to build game history"
                }
              />
              <DetailStatCard
                title="Peak Day"
                value={
                  peakDay && peakDay.activeMs > 0
                    ? formatCompactDuration(peakDay.activeMs)
                    : "0m"
                }
                hint={
                  peakDay && peakDay.activeMs > 0
                    ? `${formatCalendarDay(peakDay.key)} was the busiest recent day`
                    : "No recent active day yet"
                }
              />
            </div>
          </div>
        </div>
      </section>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(320px,0.8fr)]">
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <PlayCircle className="h-4 w-4 text-primary" />
              Recent Activity
            </CardTitle>
            <CardDescription>
              Last 14 days of tracked runtime with the active portion highlighted.
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
              <Clock3 className="h-4 w-4 text-primary" />
              Tracking Snapshot
            </CardTitle>
            <CardDescription>
              Quick read on how this title has been tracked so far.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Active Ratio
              </p>
              <div className="mt-3 h-3 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-[color:var(--color-chart-1)]"
                  style={{ width: `${Math.max(4, totals.activeRatio * 100)}%` }}
                />
              </div>
              <p className="mt-3 text-xs text-muted-foreground">
                {Math.round(totals.activeRatio * 100)}% of runtime stayed active.
              </p>
            </div>

            <div className="grid gap-3 sm:grid-cols-2 xl:grid-cols-1">
              <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Active Time
                </p>
                <p className="mt-2 text-xl font-semibold">
                  {formatCompactDuration(totals.activeMs)}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Idle Time
                </p>
                <p className="mt-2 text-xl font-semibold">
                  {formatCompactDuration(totals.idleMs)}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Current Status
                </p>
                <p className="mt-2 text-xl font-semibold">
                  {runningNow ? "Running now" : "Not running"}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Session Average
                </p>
                <p className="mt-2 text-xl font-semibold">
                  {formatCompactDuration(totals.averageActiveMs)}
                </p>
              </div>
            </div>

            <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-xs leading-6 text-muted-foreground">
              Vaultime derives totals from append-only session records, so this
              page reflects every tracked play session rather than a mutable counter.
            </div>
          </CardContent>
        </Card>
      </div>

      <Card className="border border-border/70">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Timer className="h-4 w-4 text-primary" />
            Session History
          </CardTitle>
          <CardDescription>
            Every tracked session for {game.title}, newest first.
          </CardDescription>
        </CardHeader>
        <CardContent>
          <SessionTimeline
            sessions={sessions}
            showGameName={false}
            emptyMessage="This game has no tracked sessions yet."
          />
        </CardContent>
      </Card>
    </div>
  );
}
