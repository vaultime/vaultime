// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useEffectEvent } from "react";
import { useMatch } from "react-router";
import { useAppearance } from "@/features/appearance/appearance-context";
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

/** Tells the look which game is in front, for its art behind the pages. */
export function useShowGameInFront() {
  const { summaries, visible } = useLibrary();
  const playing = usePlaying();
  const page = useMatch("/library/:gameId");
  const { showGame } = useAppearance();
  const front = gameInFront(summaries, visible, playing, page?.params.gameId);
  const cover = front?.cover ?? null;
  const colors = front?.tint.colors ?? [];
  const key = front ? `${cover}|${colors.map((color) => `${color.hue}/${color.chroma}`).join(",")}` : "";
  const report = useEffectEvent(() => showGame(front ? { cover, colors } : null));
  useEffect(() => {
    report();
  }, [key]);
}
