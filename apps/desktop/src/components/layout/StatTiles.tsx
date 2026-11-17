// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { cn } from "@/lib/utils";

export interface StatTile {
  label: string;
  value: string;
  note: string;
  /** Shows the value in the accent. */
  accent?: boolean;
}

// Hairlines and padding per cell, for two columns and for four from lg on.
const CELL_BORDERS = [
  "border-r border-b lg:border-b-0",
  "border-b pl-6 lg:border-b-0 lg:border-r",
  "border-r lg:pl-6",
  "pl-6",
];

/** "17h 06m" with the units smaller than the figures, so the number reads first. */
export function Figures({ text }: { text: string }) {
  return text.split(/(\d[\d.,]*)/).map((part, index) => {
    const unit = part.trim();
    if (/^\d/.test(part) || !unit) return part.trim() ? <span key={index}>{part}</span> : null;
    return (
      <span
        key={index}
        // A word like days stands apart, a letter like h sits close.
        className={cn("mr-[0.22em] text-[0.62em] font-medium tracking-normal opacity-75", unit.length > 1 ? "ml-[0.2em]" : "ml-[0.06em]")}
      >
        {unit}
      </span>
    );
  });
}

/** Four numbers in a row, separated by hairlines. */
export function StatTiles({ label, tiles }: { label: string; tiles: StatTile[] }) {
  return (
    <section aria-label={label} className="grid grid-cols-2 border-b border-rule px-8 lg:grid-cols-4 xl:px-14">
      {tiles.map((tile, index) => (
        <div key={tile.label} className={cn("min-w-0 border-rule py-6 pr-6", CELL_BORDERS[index])}>
          <div className="label-small">{tile.label}</div>
          <div
            className={cn(
              "mt-2 text-[clamp(22px,2.2vw,28px)] font-semibold tracking-[-0.01em] tabular-nums",
              tile.accent && "text-violet",
            )}
          >
            <Figures text={tile.value} />
          </div>
          <div className="mt-1.5 line-clamp-2 text-[13px] text-pretty text-faint">{tile.note}</div>
        </div>
      ))}
    </section>
  );
}
