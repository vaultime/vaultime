// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { AppearanceContext, type AppearanceState } from "@/features/appearance/appearance-context";
import {
  APPEARANCE_CACHE_KEY,
  APPEARANCE_SAVE_DELAY_MS,
  BACKGROUND_HEADER_OPACITY_PERCENT,
  WINDOW_LOOK_INTERVAL_MS,
} from "@/lib/constants";
import * as api from "@/lib/tauri";
import {
  DEFAULT_APPEARANCE,
  accentHues,
  appearanceFromSettings,
  appearanceSettings,
  resolveMode,
  themeTokens,
  windowLook,
  withChanges,
  type Appearance,
  type ThemeMode,
} from "@/lib/theme";

const DARK_QUERY = "(prefers-color-scheme: dark)";

/**
 * What `public/theme-boot.js` puts on the page before the first paint. It
 * keeps the tokens of both modes, so System mode follows the system at start.
 */
interface CachedLook {
  appearance: Appearance;
  tokens: Record<ThemeMode, Record<string, string>>;
}

/** The look of the last run, so the first render already has it. */
function cachedAppearance(): Appearance {
  try {
    const cached = JSON.parse(localStorage.getItem(APPEARANCE_CACHE_KEY) ?? "null") as CachedLook | null;
    if (!cached?.appearance) return DEFAULT_APPEARANCE;
    const { mode, ground, accent, dim, blur } = cached.appearance;
    return appearanceFromSettings(
      Object.fromEntries(appearanceSettings({ mode, ground, accent, dim, blur })),
    );
  } catch {
    return DEFAULT_APPEARANCE;
  }
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
  const [appearance, setAppearance] = useState<Appearance>(cachedAppearance);
  const [systemDark, setSystemDark] = useState(systemIsDark);
  const [background, setBackground] = useState<string | null>(null);
  // Drags wait a moment before they are saved, so a slider saves once.
  const pending = useRef(new Map<string, { value: string; timer: number }>());
  // Parts the player changed, which stored settings loaded later must not undo.
  const changed = useRef(new Set<keyof Appearance>());

  useEffect(() => {
    let cancelled = false;
    api
      .listSettings()
      .then((settings) => {
        if (cancelled) return;
        const loaded = appearanceFromSettings(Object.fromEntries(settings.map((setting) => [setting.key, setting.value])));
        setAppearance((current) => withChanges(loaded, current, changed.current));
      })
      .catch(() => {});
    api
      .getBackgroundImage()
      .then((picture) => {
        if (!cancelled) setBackground(picture);
      })
      .catch(() => {});
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
        api.setSetting(key, value).catch(() => {});
      }
      waiting.clear();
    };
  }, []);

  const mode = resolveMode(appearance.mode, systemDark);
  const tokens = useMemo(
    () => themeTokens(mode, appearance.ground, appearance.accent),
    [mode, appearance.ground, appearance.accent],
  );

  useLayoutEffect(() => {
    applyLook(mode, tokens);
    try {
      const cached: CachedLook = {
        appearance,
        tokens: {
          dark: themeTokens("dark", appearance.ground, appearance.accent),
          light: themeTokens("light", appearance.ground, appearance.accent),
        },
      };
      localStorage.setItem(APPEARANCE_CACHE_KEY, JSON.stringify(cached));
    } catch {
      // Without storage the next start shows the default look for a moment.
    }
  }, [appearance, mode, tokens]);

  // The icons in the tray and the taskbar and the title bar take the accent
  // and the ground, and the title bar the chosen mode. A change goes out at
  // once, a color dragged around in the picker at most once per interval and
  // once more where it stops.
  const windowLookSentAt = useRef(Number.NEGATIVE_INFINITY);
  useEffect(() => {
    const send = () => {
      windowLookSentAt.current = performance.now();
      api.setWindowLook(windowLook(mode, appearance.ground, appearance.accent, appearance.mode)).catch(() => {});
    };
    const wait = windowLookSentAt.current + WINDOW_LOOK_INTERVAL_MS - performance.now();
    if (wait <= 0) {
      send();
      return;
    }
    const timer = window.setTimeout(send, wait);
    return () => window.clearTimeout(timer);
  }, [mode, appearance.ground, appearance.accent, appearance.mode]);

  useLayoutEffect(() => {
    document.documentElement.style.setProperty(
      "--header-alpha",
      background ? `${BACKGROUND_HEADER_OPACITY_PERCENT}%` : "100%",
    );
  }, [background]);

  const change = useCallback((next: Partial<Appearance>, { gradual = false }: { gradual?: boolean } = {}) => {
    setAppearance((current) => ({ ...current, ...next }));
    for (const part of Object.keys(next) as (keyof Appearance)[]) changed.current.add(part);
    for (const [key, value] of appearanceSettings(next)) {
      const waiting = pending.current.get(key);
      if (waiting) window.clearTimeout(waiting.timer);
      pending.current.delete(key);
      const save = () => {
        pending.current.delete(key);
        api.setSetting(key, value).catch(() => {});
      };
      if (gradual) pending.current.set(key, { value, timer: window.setTimeout(save, APPEARANCE_SAVE_DELAY_MS) });
      else save();
    }
  }, []);

  const chooseBackground = useCallback(async (path: string) => {
    setBackground(await api.setBackgroundImage(path));
  }, []);

  const pickGameBackground = useCallback(async (gameId: string) => {
    setBackground(await api.setBackgroundFromGame(gameId));
  }, []);

  const clearBackground = useCallback(async () => {
    await api.clearBackgroundImage();
    setBackground(null);
  }, []);

  const value = useMemo<AppearanceState>(
    () => ({
      appearance,
      mode,
      accentHues: accentHues(appearance.accent),
      background,
      change,
      chooseBackground,
      pickGameBackground,
      clearBackground,
    }),
    [appearance, mode, background, change, chooseBackground, pickGameBackground, clearBackground],
  );

  return <AppearanceContext.Provider value={value}>{children}</AppearanceContext.Provider>;
}
