// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Check, ChevronDown } from "lucide-react";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import type { Game } from "@/lib/types";

/** Which game of this PC a game of another PC counts as, or none. */
export function GameLinkPicker({
  games,
  value,
  suggested,
  suggestedBecause,
  onChange,
}: {
  /** Games of this PC to pick from. */
  games: Game[];
  value: string | null;
  suggested?: string | null;
  suggestedBecause?: "launcher" | "title" | null;
  onChange: (gameId: string | null) => void;
}) {
  const sorted = [...games].sort((a, b) => a.title.localeCompare(b.title));
  const chosen = games.find((game) => game.id === value);
  const hint = suggestedBecause === "launcher" ? "same launcher id" : "same title";
  const ordered = suggested
    ? [...sorted.filter((game) => game.id === suggested), ...sorted.filter((game) => game.id !== suggested)]
    : sorted;
  return (
    <DropdownMenu>
      <DropdownMenuTrigger className="inline-flex h-9 max-w-64 min-w-0 items-center gap-2 rounded-md border border-rule px-3 text-[13px] text-text transition-colors hover:bg-glint/5 focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none">
        <span className="truncate">{chosen ? `Same as ${chosen.title}` : "Its own game"}</span>
        <ChevronDown className="size-3.5 shrink-0 opacity-70" />
      </DropdownMenuTrigger>
      <DropdownMenuContent className="max-h-72 w-auto min-w-56 overflow-y-auto">
        <DropdownMenuItem onClick={() => onChange(null)}>
          Its own game
          {value === null && <Check className="ml-auto size-4 text-violet" />}
        </DropdownMenuItem>
        {ordered.map((game) => (
          <DropdownMenuItem key={game.id} onClick={() => onChange(game.id)}>
            <span className="truncate">Same as {game.title}</span>
            {game.id === suggested && <span className="text-[12px] text-faint">{hint}</span>}
            {game.id === value && <Check className="ml-auto size-4 shrink-0 text-violet" />}
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}
