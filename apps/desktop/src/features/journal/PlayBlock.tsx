// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { CSSProperties } from "react";
import type { PlayRun } from "@/lib/types";
import { cn } from "@/lib/utils";
import { BLEED_PX, BLEND_STRETCH, hatch, leaningClip, placeOnDay, type Place } from "./strip";

/**
 * Sessions that overlap as one rounded block, in the fill of the game that
 * ran alone and in stripes where games ran side by side. Every change leans
 * with the stripes, so the block reads as one piece of play however many
 * games come and go.
 */
export function PlayBlock({
  run,
  day,
  fillOf,
  markOf,
  openStart,
  openEnd,
}: {
  run: PlayRun;
  day: Date;
  fillOf: (gameId: string) => string;
  markOf: (gameId: string) => string;
  /** Play went on from the day before, so the block runs in from the left edge. */
  openStart: boolean;
  /** Play goes on past midnight, so the block runs out at the right edge. */
  openEnd: boolean;
}) {
  const block = placeOnDay(new Date(run.start_ms), new Date(run.end_ms), day);
  const layer = (box: Place): CSSProperties => ({
    left: `${((box.left - block.left) / block.width) * 100}%`,
    width: `${(box.width / block.width) * 100}%`,
  });
  // Stripes come from layers as wide as the strip, so they run on from one
  // stretch to the next and keep their spot when a game joins or leaves. A
  // game's fill spans its sessions, so two colored art stays one gradient.
  const strip: Place = { left: 0, width: 100 };
  // Stripes go on top, so a short stretch side by side keeps its narrowest width.
  const pieces = [...run.pieces].sort((a, b) => Number(a.game_ids.length > 1) - Number(b.game_ids.length > 1));
  // At midnight the block keeps going out of the strip and fades, in the
  // colors of the games that ran on, so it reads as cut by the day, not ended.
  // A fill in two colors shows the end of its blend on that side.
  const bleed = (side: "left" | "right", gameIds: string[]) => {
    const fill = fillOf(gameIds[0]);
    const blend = fill === markOf(gameIds[0]) ? undefined : fill;
    return (
      <span
        className="absolute inset-y-0"
        style={{
          [side]: `-${BLEED_PX}px`,
          width: BLEED_PX,
          backgroundColor: markOf(gameIds[0]),
          backgroundImage: gameIds.length > 1 ? hatch(gameIds.map(markOf)) : blend,
          backgroundSize: gameIds.length > 1 ? undefined : BLEND_STRETCH,
          backgroundPosition: side,
          maskImage: `linear-gradient(to ${side}, black, transparent)`,
        }}
      />
    );
  };
  return (
    <>
      {openStart && bleed("left", run.pieces[0].game_ids)}
      {openEnd && bleed("right", run.pieces[run.pieces.length - 1].game_ids)}
      <span
        className={cn("absolute inset-y-0 overflow-hidden", !openStart && "rounded-l-full", !openEnd && "rounded-r-full")}
        style={{ left: `${block.left}%`, width: `${block.width}%` }}
      >
        {pieces.map((piece) => {
          const alone = piece.game_ids.length === 1;
          const box = alone
            ? placeOnDay(new Date(piece.sessions_start_ms), new Date(piece.sessions_end_ms), day)
            : strip;
          const place = placeOnDay(new Date(piece.start_ms), new Date(piece.end_ms), day);
          return (
            <span
              key={piece.start_ms}
              className="absolute inset-y-0"
              style={{
                ...layer(box),
                background: alone ? fillOf(piece.game_ids[0]) : hatch(piece.game_ids.map(markOf)),
                clipPath: leaningClip(box, place, piece.start_ms > run.start_ms, piece.end_ms < run.end_ms),

              }}
            />
          );
        })}
      </span>
    </>
  );
}
