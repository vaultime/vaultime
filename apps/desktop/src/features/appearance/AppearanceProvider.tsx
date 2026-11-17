// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  AppearanceContext,
  type AppearanceState,
  type GameInFront,
  type GamesInFront,
} from "@/features/appearance/appearance-context";
import { useSettings } from "@/features/settings/settings-context";
import {
  APPEARANCE_CACHE_KEY,
  APPEARANCE_SAVE_DELAY_MS,
  BACKGROUND_BRIGHT_DIM_PER_LIGHTNESS,
  BACKGROUND_BRIGHT_LIGHTNESS,
  BACKGROUND_DIM_PERCENT,
  BACKGROUND_HEADER_OPACITY_PERCENT,
  BACKGROUND_PANEL_OPACITY_PERCENT,
  GAME_ACCENT,
  WINDOW_LOOK_INTERVAL_MS,
} from "@/lib/constants";
import { averageLightness } from "@/lib/game-tint";
import * as api from "@/lib/tauri";
import {
  accentHues,
  accentInUse,
  appearanceFromSettings,
  appearanceSettings,
  resolveMode,
  themeTokens,
  windowLook,
  type Appearance,
  type ThemeMode,
} from "@/lib/theme";
import { logFailure } from "@/lib/utils";

const DARK_QUERY = "(prefers-color-scheme: dark)";

/**
 * What `public/theme-boot.js` puts on the page before the first paint. It
 * keeps the tokens of both modes, so System mode follows the system at start.
 */
interface CachedLook {
  appearance: Appearance;
  tokens: Record<ThemeMode, Record<string, string>>;
  /** The games in front then, so a start shows their art before the library is read. */
  games?: GamesInFront;
}

const NO_GAMES: GamesInFront = { page: null, home: null };

/** The game played last in the last run, also as the page's, as a start opens Home. */
function cachedGames(): GamesInFront {
  try {
    const cached = JSON.parse(localStorage.getItem(APPEARANCE_CACHE_KEY) ?? "null") as Partial<CachedLook> | null;
    const home = cached?.games?.home;
    return isGameInFront(home) ? { page: home, home } : NO_GAMES;
  } catch {
    return NO_GAMES;
  }
}

function isGameInFront(value: unknown): value is GameInFront {
  const game = value as Partial<GameInFront> | null | undefined;
  return (
    typeof game === "object" &&
    game !== null &&
    (game.cover === null || typeof game.cover === "string") &&
    Array.isArray(game.colors) &&
    game.colors.every((color) => typeof color?.hue === "number" && typeof color?.chroma === "number")
  );
}

function sameGame(a: GameInFront | null, b: GameInFront | null): boolean {
  if (!a || !b) return a === b;
  return (
    a.cover === b.cover &&
    a.colors.length === b.colors.length &&
    a.colors.every((color, index) => color.hue === b.colors[index].hue && color.chroma === b.colors[index].chroma)
  );
}

function systemIsDark(): boolean {
  try {
    return window.matchMedia(DARK_QUERY).matches;
  } catch {
    return true;
  }
}

/**
 * Puts the tokens on the page root, as `theme-boot.js` does at start. Hover
 * transitions stay off for that frame, so the whole page switches at once.
 */
function applyLook(mode: ThemeMode, tokens: Record<string, string>) {
  const root = document.documentElement;
  root.classList.add("appearance-switch");
  for (const [name, value] of Object.entries(tokens)) root.style.setProperty(name, value);
  root.style.colorScheme = mode;
  root.classList.toggle("dark", mode === "dark");
  requestAnimationFrame(() => requestAnimationFrame(() => root.classList.remove("appearance-switch")));
}

/** Dark or light, a ground tone, an accent and a background picture, for the whole app. */
export function AppearanceProvider({ children }: { children: ReactNode }) {
  const { values, save, reloads } = useSettings();
  const [appearance, setAppearance] = useState<Appearance>(() => appearanceFromSettings(values));
  // A restore onto a PC that never chose a look brings the look of the backup.
  const [seenReloads, setSeenReloads] = useState(reloads);
  if (reloads !== seenReloads) {
    setSeenReloads(reloads);
    setAppearance(appearanceFromSettings(values));
  }
  const [systemDark, setSystemDark] = useState(systemIsDark);
  const [background, setBackground] = useState<string | null>(null);
  const [games, setGames] = useState<GamesInFront>(cachedGames);
  // Until the library tells which games are in front, the window keeps the look the core restored.
  const [gamesKnown, setGamesKnown] = useState(false);
  const [lightness, setLightness] = useState<{ of: string; value: number | null } | null>(null);
  // Drags wait a moment before they are saved, so a slider saves once.
  const pending = useRef(new Map<string, { value: string; timer: number }>());

  useEffect(() => {
    let cancelled = false;
    api
      .getBackgroundImage()
      .then((picture) => {
        if (!cancelled) setBackground(picture);
      })
      .catch(logFailure("loading the background picture"));
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    const query = window.matchMedia(DARK_QUERY);
    const follow = () => setSystemDark(query.matches);
    query.addEventListener("change", follow);
    return () => query.removeEventListener("change", follow);
  }, []);

  // A change still waiting when the app closes is saved right away.
  useEffect(() => {
    const waiting = pending.current;
    return () => {
      for (const [key, { value, timer }] of waiting) {
        window.clearTimeout(timer);
        save(key, value).catch(logFailure(`saving ${key}`));
      }
      waiting.clear();
    };
  }, [save]);

  const mode = resolveMode(appearance.mode, systemDark);
  const gameAccent = accentInUse(GAME_ACCENT, games.page?.colors ?? []);
  const accent = appearance.accent === GAME_ACCENT ? gameAccent : appearance.accent;
  // The window, its icons and the next start, which opens Home, take the game
  // playing or played last, so the tray and the shortcuts keep their color
  // while you browse.
  const windowAccent =
    appearance.accent === GAME_ACCENT ? accentInUse(GAME_ACCENT, games.home?.colors ?? []) : appearance.accent;
  const tokens = useMemo(() => themeTokens(mode, appearance.ground, accent), [mode, appearance.ground, accent]);

  useLayoutEffect(() => {
    applyLook(mode, tokens);
    try {
      const cached: CachedLook = {
        appearance,
        tokens: {
          dark: themeTokens("dark", appearance.ground, windowAccent),
          light: themeTokens("light", appearance.ground, windowAccent),
        },
        games,
      };
      localStorage.setItem(APPEARANCE_CACHE_KEY, JSON.stringify(cached));
    } catch {
      // Without storage the next start shows the default look for a moment.
    }
  }, [appearance, windowAccent, games, mode, tokens]);

  // The icons in the tray and the taskbar and the title bar take the accent
  // and the ground, and the title bar the chosen mode. A change goes out at
  // once, a color dragged around in the picker at most once per interval and
  // once more where it stops.
  const windowLookSentAt = useRef(Number.NEGATIVE_INFINITY);
  const waitForGames = appearance.accent === GAME_ACCENT && !gamesKnown;
  useEffect(() => {
    if (waitForGames) return;
    const send = () => {
      windowLookSentAt.current = performance.now();
      api
        .setWindowLook(windowLook(mode, appearance.ground, windowAccent, appearance.mode))
        .catch(logFailure("coloring the window and its icons"));
    };
    const wait = windowLookSentAt.current + WINDOW_LOOK_INTERVAL_MS - performance.now();
    if (wait <= 0) {
      send();
      return;
    }
    const timer = window.setTimeout(send, wait);
    return () => window.clearTimeout(timer);
  }, [mode, appearance.ground, windowAccent, appearance.mode, waitForGames]);

  const backdrop =
    appearance.source === "game" ? (games.page?.cover ?? null) : appearance.source === "picture" ? background : null;

  useEffect(() => {
    if (!backdrop) return;
    let cancelled = false;
    void averageLightness(backdrop).then((value) => {
      if (!cancelled) setLightness({ of: backdrop, value });
    });
    return () => {
      cancelled = true;
    };
  }, [backdrop]);

  // Light art takes more dim, so text on the panels keeps its contrast.
  const known = lightness?.of === backdrop ? (lightness?.value ?? null) : null;
  const brightness = Math.max(0, (known ?? 0) - BACKGROUND_BRIGHT_LIGHTNESS);
  const backdropDim = Math.min(
    BACKGROUND_DIM_PERCENT.max,
    Math.round(appearance.dim + brightness * BACKGROUND_BRIGHT_DIM_PER_LIGHTNESS),
  );

  useLayoutEffect(() => {
    const root = document.documentElement.style;
    root.setProperty("--header-alpha", backdrop ? `${BACKGROUND_HEADER_OPACITY_PERCENT}%` : "100%");
    root.setProperty("--panel-alpha", backdrop ? `${BACKGROUND_PANEL_OPACITY_PERCENT}%` : "100%");
  }, [backdrop]);

  const showGames = useCallback((next: GamesInFront) => {
    setGames((current) => (sameGame(current.page, next.page) && sameGame(current.home, next.home) ? current : next));
    setGamesKnown(true);
  }, []);

  const change = useCallback(
    (next: Partial<Appearance>, { gradual = false }: { gradual?: boolean } = {}) => {
      setAppearance((current) => ({ ...current, ...next }));
      for (const [key, value] of appearanceSettings(next)) {
        const waiting = pending.current.get(key);
        if (waiting) window.clearTimeout(waiting.timer);
        pending.current.delete(key);
        const saveNow = () => {
          pending.current.delete(key);
          save(key, value).catch(logFailure(`saving ${key}`));
        };
        if (gradual) pending.current.set(key, { value, timer: window.setTimeout(saveNow, APPEARANCE_SAVE_DELAY_MS) });
        else saveNow();
      }
    },
    [save],
  );

  const chooseBackground = useCallback(
    async (path: string) => {
      setBackground(await api.setBackgroundImage(path));
      change({ source: "picture" });
    },
    [change],
  );

  const pickGameBackground = useCallback(
    async (gameId: string) => {
      setBackground(await api.setBackgroundFromGame(gameId));
      change({ source: "picture" });
    },
    [change],
  );

  const clearBackground = useCallback(async () => {
    await api.clearBackgroundImage();
    setBackground(null);
  }, []);

  const value = useMemo<AppearanceState>(
    () => ({
      appearance,
      mode,
      accentHues: accentHues(appearance.accent),
      accent,
      gameAccent,
      background,
      backdrop,
      backdropDim,
      showGames,
      change,
      chooseBackground,
      pickGameBackground,
      clearBackground,
    }),
    [
      appearance,
      mode,
      accent,
      gameAccent,
      background,
      backdrop,
      backdropDim,
      showGames,
      change,
      chooseBackground,
      pickGameBackground,
      clearBackground,
    ],
  );

  return <AppearanceContext.Provider value={value}>{children}</AppearanceContext.Provider>;
}
