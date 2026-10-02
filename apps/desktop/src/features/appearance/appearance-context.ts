// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";
import type { Appearance, ThemeMode } from "@/lib/theme";

export interface AppearanceState {
  /** What the player picked. */
  appearance: Appearance;
  /** Dark or light, with the system's choice worked out. */
  mode: ThemeMode;
  /** Hues journal marks keep free for the accent. */
  accentHues: number[];
  /** The background picture as a data URL, null without one. */
  background: string | null;
  /** Applies a change at once and saves it. */
  change: (next: Partial<Appearance>) => void;
  /** Uses a picture file as the background. */
  chooseBackground: (path: string) => Promise<void>;
  /** Uses a game's artwork as the background. */
  pickGameBackground: (gameId: string) => Promise<void>;
  clearBackground: () => Promise<void>;
}

export const AppearanceContext = createContext<AppearanceState | null>(null);

export function useAppearance(): AppearanceState {
  const value = useContext(AppearanceContext);
  if (!value) throw new Error("useAppearance needs an AppearanceProvider");
  return value;
}
