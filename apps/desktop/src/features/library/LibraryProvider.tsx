// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type { Game, Session } from "@/lib/types";
import * as api from "@/lib/tauri";
import { LibraryContext, type GameSummary } from "./library-context";

const POLL_MS = 5_000;

/** Games, sessions, covers and the live session, shared by the shell and pages. */
export function LibraryProvider({ children }: { children: ReactNode }) {
  const [games, setGames] = useState<Game[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [covers, setCovers] = useState<Record<string, string>>({});
  const [active, setActive] = useState<Session[]>([]);
  const [activePolledAt, setActivePolledAt] = useState(() => Date.now());
  const [idleThresholdSeconds, setIdleThresholdSeconds] = useState(300);
  const [loaded, setLoaded] = useState(false);
  // Ids of the running sessions at the last poll. Null until the first poll.
  const activeIds = useRef<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [nextGames, nextSessions, assets, settings] = await Promise.all([
        api.listGames(),
        api.listSessions(),
        api.listPreferredGameAssets(),
        api.listSettings(),
      ]);
      setGames(nextGames);
      setSessions(nextSessions);
      setCovers(
        Object.fromEntries(
          assets
            .filter((asset) => asset.preview_data_url)
            .map((asset) => [asset.game_id, asset.preview_data_url as string]),
        ),
      );
      const threshold = Number(settings.find((s) => s.key === "idle_threshold_seconds")?.value);
      if (Number.isFinite(threshold) && threshold > 0) setIdleThresholdSeconds(threshold);
    } finally {
      setLoaded(true);
    }
  }, []);

  useEffect(() => {
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
    const timer = setInterval(poll, POLL_MS);
    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [refresh]);

  const summaries = useMemo(() => {
    const byGame = new Map<string, GameSummary>();
    for (const game of games) {
      byGame.set(game.id, {
        game,
        cover: covers[game.id] ?? null,
        runtimeMs: 0,
        activeMs: 0,
        sessionsCount: 0,
        lastPlayedAt: null,
      });
    }
    for (const session of sessions) {
      const summary = byGame.get(session.game_id);
      if (!summary) continue;
      summary.runtimeMs += session.runtime_ms;
      summary.activeMs += session.active_ms;
      summary.sessionsCount += 1;
      if (!summary.lastPlayedAt || session.started_at_wall > summary.lastPlayedAt) {
        summary.lastPlayedAt = session.started_at_wall;
      }
    }
    return [...byGame.values()].sort((a, b) => {
      if (a.lastPlayedAt && b.lastPlayedAt) return b.lastPlayedAt.localeCompare(a.lastPlayedAt);
      if (a.lastPlayedAt) return -1;
      if (b.lastPlayedAt) return 1;
      return a.game.title.localeCompare(b.game.title);
    });
  }, [games, sessions, covers]);

  const value = useMemo(
    () => ({
      games,
      sessions,
      covers,
      active,
      activePolledAt,
      idleThresholdSeconds,
      summaries,
      loaded,
      refresh,
    }),
    [games, sessions, covers, active, activePolledAt, idleThresholdSeconds, summaries, loaded, refresh],
  );

  return <LibraryContext.Provider value={value}>{children}</LibraryContext.Provider>;
}
