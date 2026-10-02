// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { cn } from "@/lib/utils";

export interface StatTile {
  label: string;
  value: string;
  note: string;
  /** Shows the value in violet. */
  accent?: boolean;
}

// Hairlines and padding per cell, for two columns and for four from lg on.
const CELL_BORDERS = [
  "border-r border-b lg:border-b-0",
  "border-b pl-6 lg:border-b-0 lg:border-r",
  "border-r lg:pl-6",
  "pl-6",
];

/** Four numbers in a row, separated by hairlines. */
export function StatTiles({ label, tiles }: { label: string; tiles: StatTile[] }) {
  return (
    <section aria-label={label} className="grid grid-cols-2 border-b border-rule px-8 lg:grid-cols-4 xl:px-14">
      {tiles.map((tile, index) => (
        <div key={tile.label} className={cn("min-w-0 border-rule py-6 pr-6", CELL_BORDERS[index])}>
          <div className="label-caps">{tile.label}</div>
          <div
            className={cn(
              "mt-2.5 font-mono text-[clamp(22px,2.3vw,32px)] tracking-[-0.02em] tabular-nums",
              tile.accent && "text-violet",
            )}
          >
            {tile.value}
          </div>
          <div className="mt-1.5 line-clamp-2 text-[13px] text-pretty text-faint">{tile.note}</div>
        </div>
      ))}
    </section>
  );
}
