// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { listen } from "@tauri-apps/api/event";
import {
  ACTIVE_POLL_MS,
  DEFAULT_IDLE_THRESHOLD_SECS,
  LIBRARY_CHANGED_EVENT,
  MINUTE_MS,
  SETTING_KEYS,
  TRUST_BADGE_RECENT_DAYS,
} from "@/lib/constants";
import { useSettings } from "@/features/settings/settings-context";
import { tintForTitle, tintsFromImages, type GameTint } from "@/lib/game-tint";
import { compareTitles } from "@/lib/game-sort";
import { isFavorite } from "@/lib/game-metadata";
import { launcherKey, launchersOfGame } from "@/lib/launchers";
import { compareTimes, toDayKey } from "@/lib/time";
import type {
  Device,
  EarlierPlaytime,
  Game,
  GameLink,
  GameStatus,
  GameStatusChange,
  GameTotals,
  LauncherTime,
  Session,
  SessionList,
} from "@/lib/types";
import * as api from "@/lib/tauri";
import { usePageVisible } from "@/lib/use-page-visible";
import { describeError, logFailure } from "@/lib/utils";
import {
  ActiveContext,
  CountedAtContext,
  LibraryContext,
  MinuteContext,
  PlayedTodayContext,
  PlayingContext,
  type GameSummary,
} from "./library-context";

/** Games, sessions, covers and the live session, shared by the shell and pages. */
export function LibraryProvider({ children }: { children: ReactNode }) {
  const [games, setGames] = useState<Game[]>([]);
  const [sessions, setSessions] = useState<Session[]>([]);
  const [unchecked, setUnchecked] = useState<ReadonlySet<string>>(() => new Set());
  // Counts the loads, so the checked sessions of a load never replace those of a newer one.
  const loads = useRef(0);
  const [totals, setTotals] = useState<GameTotals[]>([]);
  const [launcherTimes, setLauncherTimes] = useState<LauncherTime[]>([]);
  const [earlier, setEarlier] = useState<EarlierPlaytime[]>([]);
  const [statusChanges, setStatusChanges] = useState<GameStatusChange[]>([]);
  const [notes, setNotes] = useState<Record<string, string>>({});
  const [devices, setDevices] = useState<Device[]>([]);
  const [thisDeviceId, setThisDeviceId] = useState<string | null>(null);
  const [links, setLinks] = useState<GameLink[]>([]);
  const [covers, setCovers] = useState<Record<string, string>>({});
  const [active, setActive] = useState<Session[]>([]);
  const [countedAt, setCountedAt] = useState<Record<string, number>>({});
  const [playedToday, setPlayedToday] = useState(0);
  const [today, setToday] = useState(() => toDayKey(new Date()));
  const [minute, setMinute] = useState(() => Math.floor(Date.now() / MINUTE_MS));
  const { values: settings } = useSettings();
  const savedThreshold = Number(settings[SETTING_KEYS.idleThreshold]);
  const idleThresholdSeconds =
    Number.isFinite(savedThreshold) && savedThreshold > 0 ? savedThreshold : DEFAULT_IDLE_THRESHOLD_SECS;
  const [artTints, setArtTints] = useState<Record<string, GameTint>>({});
  const [artReadFor, setArtReadFor] = useState<Record<string, string> | null>(null);
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // Ids of the running sessions at the last poll. Null until the first poll.
  const activeIds = useRef<string | null>(null);
  // The running sessions at the last poll, to keep the list while they stay the same.
  const activeJson = useRef("[]");
  const pageVisible = usePageVisible();

  const refresh = useCallback(async () => {
    const load = ++loads.current;
    const showHistory = (history: SessionList) => {
      setSessions(history.sessions);
      setTotals(history.games);
      setLauncherTimes(history.launchers);
      setUnchecked(new Set(history.unchecked));
    };
    let history: SessionList;
    try {
      // The sessions come with the chain checks the core remembers, so the
      // pages show before every chain is checked after a start.
      const [nextGames, quick, assets, nextEarlier, nextChanges, nextNotes, nextDevices, nextLinks, deviceId] =
        await Promise.all([
          api.listGames(),
          api.listSessions(TRUST_BADGE_RECENT_DAYS, false),
          api.listPreferredGameAssets(),
          api.listEarlierPlaytime(),
          api.listStatusChanges(),
          api.listSessionNotes(),
          api.listDevices(),
          api.listGameLinks(),
          api.getDeviceId(),
        ]);
      history = quick;
      setDevices(nextDevices);
      setLinks(nextLinks);
      setThisDeviceId(deviceId);
      setGames(nextGames);
      showHistory(history);
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
      return;
    } finally {
      setLoaded(true);
    }
    // Hidden, only the window look reads the library, which needs no trust
    // labels. A load after this one brings newer sessions, so it wins.
    if (history.unchecked.length === 0 || document.visibilityState !== "visible") return;
    try {
      const checked = await api.listSessions(TRUST_BADGE_RECENT_DAYS, true);
      if (load === loads.current) showHistory(checked);
    } catch (checkError) {
      logFailure("checking the chains of the sessions")(checkError);
    }
  }, []);

  // Polls only while the window is seen. The tracker runs in the core either way.
  useEffect(() => {
    if (!pageVisible) return;
    let cancelled = false;

    function poll() {
      api
        .getActiveSessions()
        .then(({ sessions: next, played_today_ms, counted_at_ms }) => {
          if (cancelled) return;
          const json = JSON.stringify(next);
          if (json !== activeJson.current) {
            activeJson.current = json;
            setActive(next);
          }
          setPlayedToday(played_today_ms);
          // Each clock runs on from the tick its session's times are of. A poll
          // before the next tick brings the same times, so no clock jumps back.
          setCountedAt(counted_at_ms);
          setMinute(Math.floor(Date.now() / MINUTE_MS));
          setToday(toDayKey(new Date()));
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
      setArtReadFor(covers);
    });
    return () => {
      cancelled = true;
    };
  }, [covers]);
  // A failed load reads no art, so the look keeps what it had.
  const artRead = loaded && error === null && artReadFor === covers;

  const summaries = useMemo(() => {
    const totalsByGame = new Map(totals.map((total) => [total.game_id, total]));
    const byGame = new Map<string, GameSummary>();
    for (const game of games) {
      // The core adds up the sessions, two installs running at once counted once.
      const total = totalsByGame.get(game.id);
      byGame.set(game.id, {
        game,
        otherTitles: [],
        favorite: isFavorite(game),
        launchers: launchersOfGame(game, links),
        launcherMs: {},
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
    for (const time of launcherTimes) {
      const summary = byGame.get(time.game_id);
      if (summary) summary.launcherMs[launcherKey(time.launcher)] = time.runtime_ms;
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
  }, [games, totals, launcherTimes, covers, artTints, earlier, statusChanges, links]);

  const setStatus = useCallback(
    async (gameId: string, status: GameStatus | "none") => {
      await api.setGameStatus(gameId, status);
      setStatusChanges(await api.listStatusChanges());
    },
    [],
  );

  const setFavorite = useCallback(async (gameId: string, favorite: boolean) => {
    const changed = await api.setGameFavorite(gameId, favorite);
    setGames((current) => current.map((game) => (game.id === changed.id ? changed : game)));
  }, []);

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
      unchecked,
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
      artRead,
      error,
      refresh,
      setStatus,
      setFavorite,
      saveNote,
    }),
    [
      games,
      sessions,
      unchecked,
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
      artRead,
      error,
      refresh,
      setStatus,
      setFavorite,
      saveNote,
    ],
  );

  return (
    <LibraryContext.Provider value={value}>
      <ActiveContext.Provider value={active}>
        <PlayingContext.Provider value={playing}>
          <CountedAtContext.Provider value={countedAt}>
            <MinuteContext.Provider value={minute}>
              <PlayedTodayContext.Provider value={playedToday}>{children}</PlayedTodayContext.Provider>
            </MinuteContext.Provider>
          </CountedAtContext.Provider>
        </PlayingContext.Provider>
      </ActiveContext.Provider>
    </LibraryContext.Provider>
  );
}
