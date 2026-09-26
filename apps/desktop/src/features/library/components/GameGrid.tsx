// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useMemo, useState } from "react";
import { Link } from "react-router";
import { MoreHorizontal, Pencil, Plus, Search, Trash2 } from "lucide-react";
import { Cover } from "@/components/media/Cover";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import type { GameSummary } from "@/features/library/library-context";
import type { Game } from "@/lib/types";
import { formatHoursShort, formatRelativeDay } from "@/lib/time";
import { cn } from "@/lib/utils";

const SORTS = {
  recent: "Recent",
  title: "A to Z",
  played: "Most played",
} as const;
type Sort = keyof typeof SORTS;

/** Every game as a cover, with sorting and the ways to add more. */
export function GameGrid({
  summaries,
  playing,
  onDiscover,
  onAdd,
  onEdit,
  onDelete,
}: {
  summaries: GameSummary[];
  playing: Set<string>;
  onDiscover: () => void;
  onAdd: () => void;
  onEdit: (game: Game) => void;
  onDelete: (game: Game) => void;
}) {
  const [sort, setSort] = useState<Sort>("recent");

  const sorted = useMemo(() => {
    if (sort === "recent") return summaries;
    const copy = [...summaries];
    if (sort === "title") copy.sort((a, b) => a.game.title.localeCompare(b.game.title));
    if (sort === "played") copy.sort((a, b) => b.runtimeMs - a.runtimeMs);
    return copy;
  }, [summaries, sort]);

  return (
    <section id="all-games" aria-labelledby="all-games-title" className="scroll-mt-6 px-8 pt-12 xl:px-14">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="flex items-baseline gap-3">
          <h2 id="all-games-title" className="font-display text-[34px] font-normal">
            All games
          </h2>
          <span className="font-mono text-sm text-faint">{summaries.length}</span>
        </div>
        <div className="flex items-center gap-2">
          <div role="radiogroup" aria-label="Sort games" className="mr-2 flex rounded-full border border-hairline p-0.5">
            {(Object.keys(SORTS) as Sort[]).map((key) => (
              <button
                key={key}
                type="button"
                role="radio"
                aria-checked={sort === key}
                onClick={() => setSort(key)}
                className={cn(
                  "h-8 rounded-full px-3.5 text-[13px] transition-colors focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none",
                  sort === key ? "bg-raised text-text" : "text-faint hover:text-soft",
                )}
              >
                {SORTS[key]}
              </button>
            ))}
          </div>
          <Button variant="outline" onClick={onDiscover}>
            <Search className="size-4" strokeWidth={1.8} />
            Discover
          </Button>
          <Button onClick={onAdd}>
            <Plus className="size-4" strokeWidth={1.8} />
            Add a game
          </Button>
        </div>
      </div>

      <ul className="mt-6 grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-x-5 gap-y-7">
        {sorted.map((summary) => (
          <GameTile
            key={summary.game.id}
            summary={summary}
            playing={playing.has(summary.game.id)}
            onEdit={onEdit}
            onDelete={onDelete}
          />
        ))}
      </ul>
    </section>
  );
}

function GameTile({
  summary,
  playing,
  onEdit,
  onDelete,
}: {
  summary: GameSummary;
  playing: boolean;
  onEdit: (game: Game) => void;
  onDelete: (game: Game) => void;
}) {
  const { game, cover, runtimeMs, lastPlayedAt, suspiciousCount, recoveredCount } = summary;

  return (
    <li className="group relative">
      <Link
        to={`/library/${game.id}`}
        className="flex flex-col gap-2.5 rounded-md focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:ring-offset-4 focus-visible:ring-offset-ink focus-visible:outline-none"
      >
        <span className="relative transition-transform duration-300 group-hover:-translate-y-1">
          <Cover
            title={game.title}
            src={cover}
            variant="card"
            className={cn(
              "aspect-[3/4] w-full p-3.5 text-2xl",
              playing && "ring-2 ring-violet ring-offset-2 ring-offset-ink",
            )}
          />
          {(suspiciousCount > 0 || recoveredCount > 0) && (
            <Badge
              variant={suspiciousCount > 0 ? "amber" : "sky"}
              className="absolute top-2 left-2 bg-ink/75 backdrop-blur-sm"
              title={
                suspiciousCount > 0
                  ? `${suspiciousCount} session${suspiciousCount === 1 ? "" : "s"} with a clock jump or timing drift`
                  : `${recoveredCount} session${recoveredCount === 1 ? "" : "s"} rebuilt after an unclean exit`
              }
            >
              {suspiciousCount > 0 ? "Suspicious" : "Recovered"}
            </Badge>
          )}
        </span>
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="truncate text-sm text-text">{game.title}</span>
          <span className="flex items-center gap-1.5 truncate text-xs text-faint">
            {playing ? (
              <>
                <span className="size-1.5 rounded-full bg-violet" />
                <span className="text-violet">Playing now</span>
              </>
            ) : runtimeMs > 0 ? (
              <>
                <span className="tabular-nums">{formatHoursShort(runtimeMs)}</span>
                {lastPlayedAt && <span aria-hidden="true">·</span>}
                {lastPlayedAt && <span className="truncate">{formatRelativeDay(lastPlayedAt)}</span>}
              </>
            ) : (
              "Not played yet"
            )}
          </span>
        </span>
      </Link>

      <DropdownMenu>
        <DropdownMenuTrigger
          aria-label={`Options for ${game.title}`}
          className="absolute top-2 right-2 flex size-8 items-center justify-center rounded-full bg-ink/70 text-soft opacity-0 backdrop-blur-sm transition-opacity group-hover:opacity-100 hover:text-text focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:outline-none data-popup-open:opacity-100"
        >
          <MoreHorizontal className="size-4" strokeWidth={1.8} />
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onClick={() => onEdit(game)}>
            <Pencil className="size-4" />
            Edit
          </DropdownMenuItem>
          <DropdownMenuItem variant="destructive" onClick={() => onDelete(game)}>
            <Trash2 className="size-4" />
            Delete
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </li>
  );
}
