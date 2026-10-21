// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ACTIVE_POLL_MS,
  DEFAULT_IDLE_THRESHOLD_SECS,
  LIBRARY_CHANGED_EVENT,
  SETTING_KEYS,
  TRUST_BADGE_RECENT_DAYS,
} from "@/lib/constants";
import { useSettings } from "@/features/settings/settings-context";
import { tintForTitle, tintsFromImages, type GameTint } from "@/lib/game-tint";
import { compareTitles } from "@/lib/game-sort";
import { compareTimes, toDayKey } from "@/lib/time";
import type {
  Device,
  EarlierPlaytime,
  Game,
  GameLink,
  GameStatus,
  GameStatusChange,
  GameTotals,
  Session,
} from "@/lib/types";
import * as api from "@/lib/tauri";
import { usePageVisible } from "@/lib/use-page-visible";
import { describeError } from "@/lib/utils";
import {
  ActiveContext,
  LastPollContext,
  LibraryContext,
  PlayedTodayContext,
  PlayingContext,
  type GameSummary,
} from "./library-context";

/** Games, sessions, covers and the live session, shared by the shell and pages. */
export function LibraryProvider({ children }: { children: ReactNode }) {
  const [games, setGames] = useState<Game[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [totals, setTotals] = useState<GameTotals[]>([]);
  const [earlier, setEarlier] = useState<EarlierPlaytime[]>([]);
  const [statusChanges, setStatusChanges] = useState<GameStatusChange[]>([]);
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [devices, setDevices] = useState<Device[]>([]);
  const [thisDeviceId, setThisDeviceId] = useState<string | null>(null);
  const [links, setLinks] = useState<GameLink[]>([]);
  const [covers, setCovers] = useState<Record<string, string>>({});
  const [active, setActive] = useState<Session[]>([]);
  const [lastPoll, setLastPoll] = useState(() => Date.now());
  const [playedToday, setPlayedToday] = useState(0);
  const [today, setToday] = useState(() => toDayKey(new Date()));
  const { values: settings } = useSettings();
  const savedThreshold = Number(settings[SETTING_KEYS.idleThreshold]);
  const idleThresholdSeconds =
    Number.isFinite(savedThreshold) && savedThreshold > 0 ? savedThreshold : DEFAULT_IDLE_THRESHOLD_SECS;
  const [artTints, setArtTints] = useState<Record<string, GameTint>>({});
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Ids of the running sessions at the last poll. Null until the first poll.
  const activeIds = useRef<string | null>(null);
  // The running sessions at the last poll, to keep the list while they stay the same.
  const activeJson = useRef("[]");
  const pageVisible = usePageVisible();

  const refresh = useCallback(async () => {
    try {
      const [nextGames, history, assets, nextEarlier, nextChanges, nextNotes, nextDevices, nextLinks, deviceId] =
        await Promise.all([
          api.listGames(),
          api.listSessions(TRUST_BADGE_RECENT_DAYS),
          api.listPreferredGameAssets(),
          api.listEarlierPlaytime(),
          api.listStatusChanges(),
          api.listSessionNotes(),
          api.listDevices(),
          api.listGameLinks(),
          api.getDeviceId(),
        ]);
      setDevices(nextDevices);
      setLinks(nextLinks);
      setThisDeviceId(deviceId);
      setGames(nextGames);
      setSessions(history.sessions);
      setTotals(history.games);
      setEarlier(nextEarlier);
      setStatusChanges(nextChanges);
      setNotes(Object.fromEntries(nextNotes.map((note) => [note.session_id, note.note])));
      setCovers(
        Object.fromEntries(
          assets
            .filter((asset) => asset.preview_data_url)
            .map((asset) => [asset.game_id, asset.preview_data_url as string]),
        ),
      );
      setError(null);
    } catch (loadError) {
      setError(describeError(loadError));
    } finally {
      setLoaded(true);
    }
  }, []);

  // Polls only while the window is seen. The tracker runs in the core either way.
  useEffect(() => {
    if (!pageVisible) return;
    let cancelled = false;
    // Polls by number, so a slow reload never brings back what an earlier poll saw.
    let polls = 0;
    let shown = 0;

    function poll() {
      polls += 1;
      const number = polls;
      api
        .getActiveSessions()
        .then(({ sessions: next, played_today_ms }) => {
          if (cancelled) return;
          const polledAt = Date.now();
          const show = () => {
            if (cancelled || number < shown) return;
            shown = number;
            const json = JSON.stringify(next);
            if (json !== activeJson.current) {
              activeJson.current = json;
              setActive(next);
            }
            setPlayedToday(played_today_ms);
            setLastPoll(polledAt);
          };
          setToday(toDayKey(new Date()));
          // The first poll, or a session started or ended: totals changed.
          const ids = next.map((s) => s.id).sort().join(",");
          if (ids !== activeIds.current) {
            activeIds.current = ids;
            // Totals first, so a game that stops leaves the live views with its new last played.
            refresh().then(show, () => {});
          } else {
            show();
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
    tintsFromImages(covers).then((next) => {
      if (cancelled) return;
      // Tints are kept per image, so unchanged covers give the same objects
      // and a reload does not draw every page a second time.
      setArtTints((current) => {
        const keys = Object.keys(next);
        const same =
          keys.length === Object.keys(current).length && keys.every((gameId) => current[gameId] === next[gameId]);
        return same ? current : next;
      });
    });
    return () => {
      cancelled = true;
    };
  }, [covers]);

  const summaries = useMemo(() => {
    const totalsByGame = new Map(totals.map((total) => [total.game_id, total]));
    const byGame = new Map<string, GameSummary>();
    for (const game of games) {
      // The core adds up the sessions, two installs running at once counted once.
      const total = totalsByGame.get(game.id);
      byGame.set(game.id, {
        game,
        otherTitles: [],
        cover: covers[game.id] ?? null,
        tint: artTints[game.id] ?? tintForTitle(game.title),
        runtimeMs: total?.runtime_ms ?? 0,
        activeMs: total?.active_ms ?? 0,
        idleMs: total?.idle_ms ?? 0,
        longestSessionMs: total?.longest_session_ms ?? 0,
        firstPlayedAt: total?.first_played_at ?? null,
        earlier: null,
        totalMs: 0,
        status: null,
        statusSince: null,
        sessionsCount: total?.sessions_count ?? 0,
        suspiciousCount: total?.suspicious_count ?? 0,
        recoveredCount: total?.recovered_count ?? 0,
        recentSuspiciousCount: total?.recent_suspicious_count ?? 0,
        recentRecoveredCount: total?.recent_recovered_count ?? 0,
        lastPlayedAt: total?.last_played_at ?? null,
      });
    }
    for (const link of links) {
      const summary = byGame.get(link.linked_game_id);
      if (summary && link.title !== summary.game.title && !summary.otherTitles.includes(link.title)) {
        summary.otherTitles.push(link.title);
      }
    }
    for (const entry of earlier) {
      const summary = byGame.get(entry.game_id);
      if (summary) summary.earlier = entry;
    }
    for (const summary of byGame.values()) {
      summary.totalMs = summary.runtimeMs + (summary.earlier?.earlier_ms ?? 0);
    }
    // Changes come oldest first, so the last one per game is its status.
    for (const change of statusChanges) {
      const summary = byGame.get(change.game_id);
      if (!summary) continue;
      summary.status = change.status === "none" ? null : change.status;
      summary.statusSince = change.status === "none" ? null : change.changed_at;
    }
    return [...byGame.values()].sort((a, b) => {
      if (a.lastPlayedAt && b.lastPlayedAt) return compareTimes(b.lastPlayedAt, a.lastPlayedAt);
      if (a.lastPlayedAt) return -1;
      if (b.lastPlayedAt) return 1;
      return compareTitles(a.game.title, b.game.title);
    });
  }, [games, totals, covers, artTints, earlier, statusChanges, links]);

  const setStatus = useCallback(
    async (gameId: string, status: GameStatus | "none") => {
      await api.setGameStatus(gameId, status);
      setStatusChanges(await api.listStatusChanges());
    },
    [],
  );

  const saveNote = useCallback(async (sessionId: string, note: string) => {
    const saved = await api.setSessionNote(sessionId, note);
    setNotes((current) => {
      const next = { ...current };
      if (saved) next[sessionId] = saved.note;
      else delete next[sessionId];
      return next;
    });
  }, []);

  const visible = useMemo(() => summaries.filter((summary) => !summary.game.is_hidden), [summaries]);

  const playingKey = [...new Set(active.map((session) => session.game_id))].sort().join(",");
  const playing = useMemo(() => new Set(playingKey ? playingKey.split(",") : []), [playingKey]);

  const value = useMemo(
    () => ({
      games,
      sessions,
      covers,
      idleThresholdSeconds,
      summaries,
      visible,
      statusChanges,
      notes,
      devices,
      thisDeviceId,
      links,
      today,
      loaded,
      error,
      refresh,
      setStatus,
      saveNote,
    }),
    [
      games,
      sessions,
      covers,
      idleThresholdSeconds,
      summaries,
      visible,
      statusChanges,
      notes,
      devices,
      thisDeviceId,
      links,
      today,
      loaded,
      error,
      refresh,
      setStatus,
      saveNote,
    ],
  );

  return (
    <LibraryContext.Provider value={value}>
      <ActiveContext.Provider value={active}>
        <PlayingContext.Provider value={playing}>
          <LastPollContext.Provider value={lastPoll}>
            <PlayedTodayContext.Provider value={playedToday}>{children}</PlayedTodayContext.Provider>
          </LastPollContext.Provider>
        </PlayingContext.Provider>
      </ActiveContext.Provider>
    </LibraryContext.Provider>
  );
}
