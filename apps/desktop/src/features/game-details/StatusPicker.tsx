// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Check, ChevronDown, Plus, X } from "lucide-react";
import { GameStatusIcon } from "@/components/status/GameStatusIcon";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { GAME_STATUS_LABELS, GAME_STATUSES } from "@/lib/game-status";
import type { GameTint } from "@/lib/game-tint";
import { formatCalendarDay } from "@/lib/time";
import type { GameStatus } from "@/lib/types";

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
        className="inline-flex h-9 items-center gap-2 rounded-full border px-4 text-sm font-medium transition-colors hover:bg-glint/5 focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
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
