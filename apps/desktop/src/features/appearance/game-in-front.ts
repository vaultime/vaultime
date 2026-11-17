// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useEffectEvent } from "react";
import { useMatch } from "react-router";
import { useAppearance, type GameInFront } from "@/features/appearance/appearance-context";
import { useLibrary, usePlaying, type GameSummary } from "@/features/library/library-context";
import { usePageVisible } from "@/lib/use-page-visible";

const NOTHING_PLAYING: ReadonlySet<string> = new Set();

/**
 * The game a page is about, else the one the player picked for the other
 * pages while it is in the library, else the one playing, else the one played
 * last, as Home shows it.
 */
export function gameInFront(
  summaries: GameSummary[],
  visible: GameSummary[],
  playing: ReadonlySet<string>,
  pageGameId: string | undefined,
  pickedGameId: string | null = null,
): GameSummary | null {
  if (pageGameId) return summaries.find((summary) => summary.game.id === pageGameId) ?? null;
  return (
    summaries.find((summary) => summary.game.id === pickedGameId) ??
    visible.find((summary) => playing.has(summary.game.id)) ??
    visible.find((summary) => summary.lastPlayedAt) ??
    null
  );
}

/**
 * The art of a game for the look. Only a tint read from the cover has a
 * lightness, so a game without art, which has the color of its title, lights
 * nothing.
 */
export function gameArt(summary: GameSummary | null): GameInFront | null {
  if (!summary) return null;
  const { lightness, colors } = summary.tint;
  return { cover: summary.art, colors: lightness === undefined ? [] : colors, lightness: lightness ?? null };
}

/** Tells games and their art apart without spelling out the cover, a long data URL. */
function artKey(summary: GameSummary | null): string {
  const art = gameArt(summary);
  if (!summary || !art) return "";
  return `${summary.game.id}|${art.cover}|${art.lightness}|${art.colors.map((color) => `${color.hue}/${color.chroma}`).join(",")}`;
}

/**
 * Tells the look which games are in front, once the library and the colors
 * of its covers are read, so a start never shows the color of a title first.
 */
export function useShowGameInFront() {
  const { summaries, visible, artRead } = useLibrary();
  // Running sessions are polled only while the page is seen, so a hidden
  // page goes by the library, which the core reloads when one starts or ends.
  const seen = usePageVisible();
  const polled = usePlaying();
  const playing = seen ? polled : NOTHING_PLAYING;
  const page = useMatch("/library/:gameId");
  const { appearance, showGames } = useAppearance();
  const front = gameInFront(summaries, visible, playing, page?.params.gameId, appearance.game);
  const home = gameInFront(summaries, visible, playing, undefined, appearance.game);
  const key = artRead ? `${artKey(front)}/${artKey(home)}` : null;
  const report = useEffectEvent(() => showGames({ page: gameArt(front), home: gameArt(home) }));
  useEffect(() => {
    if (key !== null) report();
  }, [key]);
}
