// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";
import type { ArtColor } from "@/lib/game-tint";
import type { Appearance, ThemeMode } from "@/lib/theme";

/** A game that lights the look, by its art. */
export interface GameInFront {
  cover: string | null;
  /** The main colors of its art, the strongest first. */
  colors: ArtColor[];
  /** The average lightness of its art, from 0 to 1, null without art. */
  lightness: number | null;
}

/** The games that light the look. */
export interface GamesInFront {
  /** The game a page is about, else the one playing, else the one played last. */
  page: GameInFront | null;
  /** The game playing, else the one played last, for the window and its icons. */
  home: GameInFront | null;
}

export interface AppearanceState {
  /** What the player picked. */
  appearance: Appearance;
  /** Dark or light, with the system's choice worked out. */
  mode: ThemeMode;
  /** Hues journal marks keep free for the accent. */
  accentHues: number[];
  /** The accent the app shows, the color of the game in front worked out. */
  accent: string;
  /** The accent the game in front gives, shown on its swatch. */
  gameAccent: string;
  /** The background picture as a data URL, null without one. */
  background: string | null;
  /** What shows behind every page: the art of the game in front, the picture or nothing. */
  backdrop: string | null;
  /** The dim over the backdrop, more than the player's for light pictures. */
  backdropDim: number;
  /** Tells which games are in front, for their art and color. */
  showGames: (games: GamesInFront) => void;
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
