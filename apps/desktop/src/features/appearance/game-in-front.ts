// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useEffectEvent } from "react";
import { useMatch } from "react-router";
import { useAppearance, type GameInFront } from "@/features/appearance/appearance-context";
import { useLibrary, usePlaying, type GameSummary } from "@/features/library/library-context";

/**
 * The game a page is about, else the one playing, else the one played last,
 * as Home shows it.
 */
export function gameInFront(
  summaries: GameSummary[],
  visible: GameSummary[],
  playing: ReadonlySet<string>,
  pageGameId: string | undefined,
): GameSummary | null {
  if (pageGameId) return summaries.find((summary) => summary.game.id === pageGameId) ?? null;
  return visible.find((summary) => playing.has(summary.game.id)) ?? visible.find((summary) => summary.lastPlayedAt) ?? null;
}

function art(summary: GameSummary | null): GameInFront | null {
  return summary ? { cover: summary.cover, colors: summary.tint.colors } : null;
}

function artKey(summary: GameSummary | null): string {
  return summary ? `${summary.cover}|${summary.tint.colors.map((color) => `${color.hue}/${color.chroma}`).join(",")}` : "";
}

/**
 * Tells the look which games are in front, once the library and the colors
 * of its covers are read, so a start never shows the color of a title first.
 */
export function useShowGameInFront() {
  const { summaries, visible, artRead } = useLibrary();
  const playing = usePlaying();
  const page = useMatch("/library/:gameId");
  const { showGames } = useAppearance();
  const front = gameInFront(summaries, visible, playing, page?.params.gameId);
  const home = gameInFront(summaries, visible, playing, undefined);
  const key = artRead ? `${artKey(front)}/${artKey(home)}` : null;
  const report = useEffectEvent(() => showGames({ page: art(front), home: art(home) }));
  useEffect(() => {
    if (key !== null) report();
  }, [key]);
}
