// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import { ACTIVE_POLL_MS, DEFAULT_IDLE_THRESHOLD_SECONDS, LIBRARY_CHANGED_EVENT, SETTING_KEYS } from "@/lib/constants";
import { tintForTitle, tintFromImage, type GameTint } from "@/lib/game-tint";
import { normalizeIntegrityStatus } from "@/lib/integrity";
import type { EarlierPlaytime, Game, Session } from "@/lib/types";
import * as api from "@/lib/tauri";
import { usePageVisible } from "@/lib/use-page-visible";
import { LibraryContext, type GameSummary } from "./library-context";

/** Games, sessions, covers and the live session, shared by the shell and pages. */
export function LibraryProvider({ children }: { children: ReactNode }) {
  const [games, setGames] = useState<Game[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [earlier, setEarlier] = useState<EarlierPlaytime[]>([]);
  const [covers, setCovers] = useState<Record<string, string>>({});
  const [active, setActive] = useState<Session[]>([]);
  const [activePolledAt, setActivePolledAt] = useState(() => Date.now());
  const [idleThresholdSeconds, setIdleThresholdSeconds] = useState(DEFAULT_IDLE_THRESHOLD_SECONDS);
  const [artTints, setArtTints] = useState<Record<string, GameTint>>({});
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Ids of the running sessions at the last poll. Null until the first poll.
  const activeIds = useRef<string | null>(null);
  const pageVisible = usePageVisible();

  const refresh = useCallback(async () => {
    try {
      const [nextGames, nextSessions, assets, settings, nextEarlier] = await Promise.all([
        api.listGames(),
        api.listSessions(),
        api.listPreferredGameAssets(),
        api.listSettings(),
        api.listEarlierPlaytime(),
      ]);
      setGames(nextGames);
      setSessions(nextSessions);
      setEarlier(nextEarlier);
      setCovers(
        Object.fromEntries(
          assets
            .filter((asset) => asset.preview_data_url)
            .map((asset) => [asset.game_id, asset.preview_data_url as string]),
        ),
      );
      const threshold = Number(settings.find((s) => s.key === SETTING_KEYS.idleThreshold)?.value);
      if (Number.isFinite(threshold) && threshold > 0) setIdleThresholdSeconds(threshold);
      setError(null);
    } catch (loadError) {
      setError(String(loadError));
    } finally {
      setLoaded(true);
    }
  }, []);

  // Polls only while the window is seen. The tracker runs in the core either way.
  useEffect(() => {
    if (!pageVisible) return;
    let cancelled = false;

    function poll() {
      api
        .getActiveSessions()
        .then((next) => {
          if (cancelled) return;
          setActive(next);
          setActivePolledAt(Date.now());
          // The first poll, or a session started or ended: totals changed.
          const ids = next.map((s) => s.id).sort().join(",");
          if (ids !== activeIds.current) {
            activeIds.current = ids;
            refresh().catch(() => {});
          }
        })
        .catch(() => {
          if (!cancelled && activeIds.current === null) {
            activeIds.current = "";
            refresh().catch(() => {});
          }
        });
    }

    poll();
    const timer = setInterval(poll, ACTIVE_POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
      // Sessions may start and end unseen, so the next poll reloads the totals.
      activeIds.current = null;
    };
  }, [refresh, pageVisible]);

  // The core reports changes it makes on its own, like covers found at startup.
  useEffect(() => {
    const stop = listen(LIBRARY_CHANGED_EVENT, () => {
      refresh().catch(() => {});
    });
    return () => {
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [refresh]);

  useEffect(() => {
    let cancelled = false;
    Promise.all(
      Object.entries(covers).map(async ([gameId, src]) => [gameId, await tintFromImage(src)] as const),
    ).then((entries) => {
      if (cancelled) return;
      const next: Record<string, GameTint> = {};
      for (const [gameId, tint] of entries) {
        if (tint) next[gameId] = tint;
      }
      setArtTints(next);
    });
    return () => {
      cancelled = true;
    };
  }, [covers]);

  const summaries = useMemo(() => {
    const byGame = new Map<string, GameSummary>();
    for (const game of games) {
      byGame.set(game.id, {
        game,
        cover: covers[game.id] ?? null,
        tint: artTints[game.id] ?? tintForTitle(game.title),
        runtimeMs: 0,
        activeMs: 0,
        earlier: null,
        totalMs: 0,
        sessionsCount: 0,
        suspiciousCount: 0,
        recoveredCount: 0,
        lastPlayedAt: null,
      });
    }
    for (const session of sessions) {
      const summary = byGame.get(session.game_id);
      if (!summary) continue;
      summary.runtimeMs += session.runtime_ms;
      summary.activeMs += session.active_ms;
      summary.sessionsCount += 1;
      const trust = normalizeIntegrityStatus(session.integrity_status);
      if (trust === "suspicious") summary.suspiciousCount += 1;
      if (trust === "recovered") summary.recoveredCount += 1;
      if (!summary.lastPlayedAt || session.started_at_wall > summary.lastPlayedAt) {
        summary.lastPlayedAt = session.started_at_wall;
      }
    }
    for (const entry of earlier) {
      const summary = byGame.get(entry.game_id);
      if (summary) summary.earlier = entry;
    }
    for (const summary of byGame.values()) {
      summary.totalMs = summary.runtimeMs + (summary.earlier?.earlier_ms ?? 0);
    }
    return [...byGame.values()].sort((a, b) => {
      if (a.lastPlayedAt && b.lastPlayedAt) return b.lastPlayedAt.localeCompare(a.lastPlayedAt);
      if (a.lastPlayedAt) return -1;
      if (b.lastPlayedAt) return 1;
      return a.game.title.localeCompare(b.game.title);
    });
  }, [games, sessions, covers, artTints, earlier]);

  const visible = useMemo(() => summaries.filter((summary) => !summary.game.is_hidden), [summaries]);

  const value = useMemo(
    () => ({
      games,
      sessions,
      covers,
      active,
      activePolledAt,
      idleThresholdSeconds,
      summaries,
      visible,
      loaded,
      error,
      refresh,
    }),
    [games, sessions, covers, active, activePolledAt, idleThresholdSeconds, summaries, visible, loaded, error, refresh],
  );

  return <LibraryContext.Provider value={value}>{children}</LibraryContext.Provider>;
}
