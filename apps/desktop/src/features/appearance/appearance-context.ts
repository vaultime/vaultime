// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";
import type { ArtColor } from "@/lib/game-tint";
import type { Appearance, ThemeMode } from "@/lib/theme";

/** The game a page is about, or the one playing or played last. */
export interface GameInFront {
  cover: string | null;
  /** The main colors of its art, the strongest first. */
  colors: ArtColor[];
}

export interface AppearanceState {
  /** What the player picked. */
  appearance: Appearance;
  /** Dark or light, with the system's choice worked out. */
  mode: ThemeMode;
  /** Hues journal marks keep free for the accent. */
  accentHues: number[];
  /** The background picture as a data URL, null without one. */
  background: string | null;
  /** What shows behind every page: the art of the game in front, the picture or nothing. */
  backdrop: string | null;
  /** The dim over the backdrop, more than the player's for light pictures. */
  backdropDim: number;
  /** Tells which game is in front, for its art behind the pages. */
  showGame: (game: GameInFront | null) => void;
  /**
   * Applies a change at once and saves it. A `gradual` change, like a slider
   * being dragged, is saved once it stops.
   */
  change: (next: Partial<Appearance>, options?: { gradual?: boolean }) => void;
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
