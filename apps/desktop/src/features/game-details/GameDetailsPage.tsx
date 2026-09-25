// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { open } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  Clock3,
  Shield,
  ImagePlus,
  Loader2,
  PaintBucket,
  PlayCircle,
  RefreshCw,
  Timer,
} from "lucide-react";
import { Link, useParams } from "react-router";
import { useCallback, useEffect, useMemo, useState } from "react";
import { GameArtwork } from "@/components/media/GameArtwork";
import { IntegrityBadge } from "@/components/status/IntegrityBadge";
import { Button } from "@/components/ui/button";
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
  formatCompactDuration,
  formatLongDate,
  formatSessionDate,
  formatCalendarDay,
} from "@/lib/time";
import {
  formatIntegrityEventType,
  getIntegrityEventDetail,
  summarizeIntegrity,
} from "@/lib/integrity";
import type { Game, GameAssetView, Session, SessionEvent } from "@/lib/types";
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
  const [events, setEvents] = useState<SessionEvent[]>([]);
  const [assets, setAssets] = useState<GameAssetView[]>([]);
  const [assetBusy, setAssetBusy] = useState(false);

  const loadGameBundle = useCallback(async (currentGameId: string) => {
    const [fetchedGame, fetchedSessions, fetchedEvents, fetchedAssets] =
      await Promise.all([
      api.getGame(currentGameId),
      api.getSessionsForGame(currentGameId),
      api.getSessionEventsForGame(currentGameId),
      api.listGameAssets(currentGameId),
      ]);

    return {
      game: fetchedGame,
      sessions: fetchedSessions,
      events: fetchedEvents,
      assets: fetchedAssets,
    };
  }, []);

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
        const bundle = await loadGameBundle(currentGameId);

        if (cancelled) {
          return;
        }

        setGame(bundle.game);
        setSessions(bundle.sessions);
        setEvents(bundle.events);
        setAssets(bundle.assets);
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
  }, [gameId, loadGameBundle]);

  useEffect(() => {
    if (!gameId) {
      return;
    }

    if (!activeSessions.some((session) => session.game_id === gameId)) {
      return;
    }

    loadGameBundle(gameId)
      .then((bundle) => {
        setGame(bundle.game);
        setSessions(bundle.sessions);
        setEvents(bundle.events);
        setAssets(bundle.assets);
      })
      .catch(() => {});
  }, [activeSessions, gameId, loadGameBundle]);

  const totals = useMemo(() => getSessionTotals(sessions), [sessions]);
  const dailyActivity = useMemo(
    () => buildDailyActivity(sessions, 14),
    [sessions],
  );
  const integritySummary = useMemo(
    () => summarizeIntegrity(sessions),
    [sessions],
  );
  const notableEvents = useMemo(
    () =>
      events
        .filter((event) => event.event_type !== "heartbeat")
        .slice(0, 8),
    [events],
  );
  const heartbeatCount = useMemo(
    () => events.filter((event) => event.event_type === "heartbeat").length,
    [events],
  );
  const peakDay = useMemo(() => {
    return [...dailyActivity].sort((a, b) => b.activeMs - a.activeMs)[0] ?? null;
  }, [dailyActivity]);
  const runningNow = activeSessions.some((session) => session.game_id === gameId);
  const preferredAsset = useMemo(
    () => assets.find((asset) => asset.is_preferred) ?? assets[0] ?? null,
    [assets],
  );

  async function handleRescanArtwork() {
    if (!gameId) {
      return;
    }

    try {
      setAssetBusy(true);
      const nextAssets = await api.scanGameAssets(gameId);
      setAssets(nextAssets);
    } finally {
      setAssetBusy(false);
    }
  }

  async function handleImportArtwork() {
    if (!gameId) {
      return;
    }

    const selected = await open({
      multiple: false,
      directory: false,
      title: "Choose artwork image",
      filters: [
        {
          name: "Images",
          extensions: ["png", "jpg", "jpeg", "webp", "bmp", "ico"],
        },
      ],
    });

    if (!selected || typeof selected !== "string") {
      return;
    }

    try {
      setAssetBusy(true);
      const nextAssets = await api.importGameAsset(gameId, selected);
      setAssets(nextAssets);
    } finally {
      setAssetBusy(false);
    }
  }

  async function handleSelectPreferred(assetId: string) {
    if (!gameId) {
      return;
    }

    try {
      setAssetBusy(true);
      await api.setPreferredGameAsset(gameId, assetId);
      const nextAssets = await api.listGameAssets(gameId);
      setAssets(nextAssets);
    } finally {
      setAssetBusy(false);
    }
  }

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
        {preferredAsset?.preview_data_url && (
          <div className="absolute inset-0">
            <img
              src={preferredAsset.preview_data_url}
              alt={`${game.title} background art`}
              className="h-full w-full object-cover opacity-22 blur-[2px]"
            />
          </div>
        )}
        <div className="absolute inset-0 bg-[linear-gradient(90deg,rgba(10,6,18,0.94),rgba(10,6,18,0.68)_52%,rgba(10,6,18,0.88)),radial-gradient(circle_at_top_left,rgba(152,92,255,0.34),transparent_34%),radial-gradient(circle_at_bottom_right,rgba(82,43,179,0.28),transparent_36%)]" />
        <div className="relative grid gap-6 p-6 lg:grid-cols-[260px_minmax(0,1fr)] lg:p-8">
          <GameArtwork
            src={preferredAsset?.preview_data_url ?? null}
            alt={`${game.title} artwork`}
            className="h-72 rounded-[1.75rem] border border-white/10 shadow-[0_30px_60px_rgba(0,0,0,0.35)]"
            imageClassName="object-cover object-center"
            iconClassName="h-20 w-20"
          />

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
                  {integritySummary.totalCount > 0 && (
                    <IntegrityBadge status={integritySummary.overallStatus} />
                  )}
                </div>
                <p className="max-w-2xl text-sm leading-6 text-muted-foreground">
                  {game.executable_path ??
                    game.install_folder ??
                    "Manual game entry without an executable path yet."}
                </p>
                {integritySummary.totalCount > 0 && (
                  <p className="max-w-2xl text-xs leading-6 text-muted-foreground">
                    {integritySummary.note}
                  </p>
                )}
                <div className="flex flex-wrap gap-2 pt-2">
                  <Button
                    variant="outline"
                    onClick={handleRescanArtwork}
                    disabled={assetBusy}
                    className="bg-background/35 backdrop-blur-sm"
                  >
                    {assetBusy ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <RefreshCw className="h-4 w-4" />
                    )}
                    Rescan Artwork
                  </Button>
                  <Button
                    variant="outline"
                    onClick={handleImportArtwork}
                    disabled={assetBusy}
                    className="bg-background/35 backdrop-blur-sm"
                  >
                    <ImagePlus className="h-4 w-4" />
                    Add Override
                  </Button>
                </div>
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
              <PaintBucket className="h-4 w-4 text-primary" />
              Artwork Rack
            </CardTitle>
            <CardDescription>
              Scanned folder art and manual overrides cached for this game.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            {assets.length > 0 ? (
              <>
                <div className="grid grid-cols-2 gap-3">
                  {assets.map((asset) => (
                    <button
                      key={asset.id}
                      type="button"
                      onClick={() => handleSelectPreferred(asset.id)}
                      disabled={assetBusy}
                      className="group text-left"
                    >
                      <div
                        className={`overflow-hidden rounded-[1.4rem] border transition-all ${
                          asset.is_preferred
                            ? "border-primary shadow-[0_0_0_1px_rgba(193,151,255,0.3)]"
                            : "border-border/70 hover:border-primary/40"
                        }`}
                      >
                        <GameArtwork
                          src={asset.preview_data_url}
                          alt={`${game.title} asset`}
                          className="aspect-[4/5]"
                          imageClassName="transition-transform duration-500 group-hover:scale-[1.03]"
                        />
                      </div>
                      <div className="mt-2 px-1">
                        <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                          {asset.asset_type}
                        </p>
                        <p className="mt-1 text-xs text-muted-foreground">
                          {asset.source === "user_picked"
                            ? "Manual override"
                            : "Folder scan"}
                          {asset.is_preferred ? " · preferred" : ""}
                        </p>
                      </div>
                    </button>
                  ))}
                </div>

                <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-xs leading-6 text-muted-foreground">
                  Click any cached image to make it the preferred cover used in
                  the library and on this detail page.
                </div>
              </>
            ) : (
              <div className="rounded-2xl border border-dashed border-border/70 bg-muted/20 p-5 text-sm text-muted-foreground">
                No artwork has been cached for this game yet. Rescan the game
                folder or add a manual image override.
              </div>
            )}
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,0.95fr)_minmax(0,1.05fr)]">
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Clock3 className="h-4 w-4 text-primary" />
              Tracking Snapshot
            </CardTitle>
            <CardDescription>
              Quick read on active time, local trust, and recovery state.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Trust Status
              </p>
              <div className="mt-3 flex flex-wrap items-center gap-3">
                <IntegrityBadge status={integritySummary.overallStatus} />
                <p className="text-xs text-muted-foreground">
                  Local hash-linked audit log with optional cloud acknowledgement. Device signing is still pending.
                </p>
              </div>
            </div>

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

            <div className="grid gap-3 sm:grid-cols-2">
              <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Verified
                </p>
                <p className="mt-2 text-xl font-semibold">
                  {integritySummary.verifiedCount}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Local Only
                </p>
                <p className="mt-2 text-xl font-semibold">
                  {integritySummary.localCount}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Recovered
                </p>
                <p className="mt-2 text-xl font-semibold">
                  {integritySummary.recoveredCount}
                </p>
              </div>
              <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                  Suspicious
                </p>
                <p className="mt-2 text-xl font-semibold">
                  {integritySummary.suspiciousCount}
                </p>
              </div>
            </div>
            <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Session Average
              </p>
              <p className="mt-2 text-xl font-semibold">
                {formatCompactDuration(totals.averageActiveMs)}
              </p>
            </div>
          </CardContent>
        </Card>

        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Shield className="h-4 w-4 text-primary" />
              Integrity Audit
            </CardTitle>
            <CardDescription>
              Notable local audit events for this title, newest first.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4 text-sm text-muted-foreground">
            <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
              Session events are hash-linked locally. Heartbeat rows are stored
              in the database but hidden here unless they carry a notable state
              change.
            </div>

            {notableEvents.length === 0 ? (
              <div className="rounded-2xl border border-dashed border-border/70 bg-muted/20 p-4">
                No session audit events recorded for this title yet.
              </div>
            ) : (
              <div className="space-y-2">
                {notableEvents.map((event) => (
                  <div
                    key={event.id}
                    className="rounded-2xl border border-border/70 bg-muted/20 px-4 py-3"
                  >
                    <div className="flex items-start justify-between gap-3">
                      <div className="space-y-1">
                        <p className="text-sm font-medium text-foreground">
                          {formatIntegrityEventType(event.event_type)}
                        </p>
                        <p className="text-xs leading-5 text-muted-foreground">
                          {getIntegrityEventDetail(event)}
                        </p>
                      </div>
                      <div className="shrink-0 text-right">
                        <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                          #{event.sequence}
                        </p>
                        <p className="mt-1 text-xs text-muted-foreground">
                          {formatSessionDate(event.event_time_wall)}
                        </p>
                      </div>
                    </div>

                    <div className="mt-3 flex flex-wrap items-center gap-2 text-[11px]">
                      <span className="rounded-full border border-border/70 bg-background/40 px-2.5 py-1 text-muted-foreground">
                        {event.hash_self ? "Hash linked" : "Missing hash"}
                      </span>
                      <span className="rounded-full border border-border/70 bg-background/40 px-2.5 py-1 text-muted-foreground">
                        {event.signature ? "Signed" : "Unsigned local event"}
                      </span>
                    </div>
                  </div>
                ))}
              </div>
            )}

            {heartbeatCount > 0 && (
              <p className="text-xs leading-5 text-muted-foreground">
                {heartbeatCount} heartbeat event
                {heartbeatCount === 1 ? "" : "s"} omitted to keep the audit
                trail readable.
              </p>
            )}
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
