// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Check, ChevronDown, Heart, Plus, X } from "lucide-react";
import { GameStatusIcon } from "@/components/status/GameStatusIcon";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { GAME_STATUS_LABELS, GAME_STATUSES } from "@/lib/game-status";
import type { GameTint } from "@/lib/game-tint";
import { formatCalendarDay } from "@/lib/time";
import type { GameStatus } from "@/lib/types";

/** Makes the game a favorite or no longer, as a square in the colors of its header. */
export function FavoriteButton({
  tint,
  favorite,
  onChange,
}: {
  tint: GameTint;
  favorite: boolean;
  onChange: (favorite: boolean) => void;
}) {
  const label = favorite ? "Remove from favorites" : "Add to favorites";
  return (
    <button
      type="button"
      aria-pressed={favorite}
      aria-label={label}
      title={label}
      onClick={() => onChange(!favorite)}
      className="inline-flex size-[34px] items-center justify-center rounded-lg border transition-colors hover:bg-glint/5 focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
      style={{ borderColor: tint.edge, color: favorite ? tint.ink : tint.muted }}
    >
      <Heart className="size-4" fill={favorite ? "currentColor" : "none"} strokeWidth={1.8} />
    </button>
  );
}

/** Where the game stands, as a pill in the colors of its header. */
export function StatusPicker({
  tint,
  status,
  since,
  onChange,
}: {
  tint: GameTint;
  status: GameStatus | null;
  since: string | null;
  onChange: (status: GameStatus | "none") => void;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger
        className="inline-flex h-[34px] items-center gap-2 rounded-lg border px-3.5 text-[13px] font-medium transition-colors hover:bg-glint/5 focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
        style={{ borderColor: tint.edge, color: tint.ink }}
      >
        {status ? <GameStatusIcon status={status} className="size-3.5" /> : <Plus className="size-3.5" />}
        {status ? GAME_STATUS_LABELS[status] : "Set a status"}
        {status && since && <span style={{ color: tint.muted }}>since {formatCalendarDay(since)}</span>}
        <ChevronDown className="size-3.5 opacity-70" />
      </DropdownMenuTrigger>
      <DropdownMenuContent className="w-auto min-w-48">
        {GAME_STATUSES.map((option) => (
          <DropdownMenuItem key={option} onClick={() => onChange(option)}>
            <GameStatusIcon status={option} className="size-4" />
            {GAME_STATUS_LABELS[option]}
            {option === status && <Check className="ml-auto size-4 text-violet" />}
          </DropdownMenuItem>
        ))}
        {status && (
          <DropdownMenuItem onClick={() => onChange("none")}>
            <X className="size-4" />
            No status
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
