// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router";
import {
  ArrowDownUp,
  Check,
  ChevronDown,
  Heart,
  HeartOff,
  MoreHorizontal,
  Pencil,
  Plus,
  Rows3,
  Search,
  Trash2,
  X,
} from "lucide-react";
import { LauncherMark } from "@/components/brand/LauncherMark";
import { Cover } from "@/components/media/Cover";
import { PhraseText } from "@/components/media/PhraseText";
import { GameStatusIcon } from "@/components/status/GameStatusIcon";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { LastPlayed } from "@/features/library/components/LastPlayed";
import { useLibrary, useMinute, type GameSummary } from "@/features/library/library-context";
import { useSettings } from "@/features/settings/settings-context";
import { MINUTE_MS, SETTING_KEYS, TRUST_BADGE_RECENT_DAYS } from "@/lib/constants";
import { onFind } from "@/lib/find";
import { GAME_GROUPS, groupGames, isGameGroup, type GameGroup } from "@/lib/game-groups";
import { GAME_SORTS, isGameSort, sortGames, type GameSort } from "@/lib/game-sort";
import { GAME_STATUS_LABELS, GAME_STATUSES } from "@/lib/game-status";
import { launcherTitle, launcherWhere } from "@/lib/launchers";
import { launcherPlaytime } from "@/lib/sentences";
import { matchesGame } from "@/lib/search";
import type { Game, GameStatus } from "@/lib/types";
import { formatHoursShort } from "@/lib/time";
import { scrollMotion } from "@/lib/anchors";
import { cn, logFailure } from "@/lib/utils";
import { plural } from "@/lib/words";

/** Every game, the favorites or the games of one launcher, as covers, with sorting and the ways to add more. */
export function GameGrid({
  summaries,
  launcher,
  favorites,
  playing,
  onDiscover,
  onAdd,
  onEdit,
  onDelete,
}: {
  summaries: GameSummary[];
  /** The launcher whose games these are, null for all games. */
  launcher: string | null;
  /** Whether these are the favorites. */
  favorites: boolean;
  playing: ReadonlySet<string>;
  onDiscover: () => void;
  onAdd: () => void;
  onEdit: (game: Game) => void;
  onDelete: (game: Game) => void;
}) {
  const { values, save } = useSettings();
  const [sort, setSort] = useState<GameSort>(() => {
    const saved = values[SETTING_KEYS.librarySort];
    return isGameSort(saved) ? saved : "recent";
  });
  const [group, setGroup] = useState<GameGroup>(() => {
    const saved = values[SETTING_KEYS.libraryGroup];
    return isGameGroup(saved) ? saved : "none";
  });
  const { links, devices, thisDeviceId } = useLibrary();
  const minute = useMinute();
  const [filter, setFilter] = useState<GameStatus | "all">("all");
  const [query, setQuery] = useState("");
  const section = useRef<HTMLElement>(null);
  const search = useRef<HTMLInputElement>(null);
  const statuses = GAME_STATUSES.filter((status) => summaries.some((summary) => summary.status === status));
  const countOf = (key: GameStatus | "all") =>
    key === "all" ? summaries.length : summaries.filter((summary) => summary.status === key).length;
  const shownFilter = filter !== "all" && statuses.includes(filter) ? filter : "all";

  const sorted = useMemo(() => {
    const filtered = summaries.filter(
      (summary) =>
        (shownFilter === "all" || summary.status === shownFilter) &&
        matchesGame(summary.game.title, summary.otherTitles, query),
    );
    return sortGames(filtered, sort);
  }, [summaries, sort, shownFilter, query]);
  // One launcher's games all fall under it, and with one PC all games are this PC's.
  const groupings = (Object.keys(GAME_GROUPS) as GameGroup[]).filter(
    (option) => (option !== "launcher" || !launcher) && (option !== "pc" || devices.length > 1),
  );
  const shownGroup = groupings.includes(group) ? group : "none";
  const groups = useMemo(
    () => groupGames(sorted, shownGroup, { now: minute * MINUTE_MS, links, devices, thisDeviceId }),
    [sorted, shownGroup, minute, links, devices, thisDeviceId],
  );

  function changeSort(next: GameSort) {
    setSort(next);
    save(SETTING_KEYS.librarySort, next).catch(logFailure("saving the sort of All games"));
  }

  function changeGroup(next: GameGroup) {
    setGroup(next);
    save(SETTING_KEYS.libraryGroup, next).catch(logFailure("saving the groups of All games"));
  }

  useEffect(
    () =>
      onFind(() => {
        section.current?.scrollIntoView({ block: "start", behavior: scrollMotion() });
        search.current?.focus({ preventScroll: true });
        search.current?.select();
      }),
    [],
  );

  return (
    <section
      ref={section}
      id="all-games"
      aria-labelledby="all-games-title"
      className="scroll-mt-6 px-8 pt-6 xl:px-14"
    >
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div className="flex items-baseline gap-3">
          {launcher && <LauncherMark launcher={launcher} className="size-[22px] shrink-0 self-center text-soft" />}
          {favorites && <Heart className="size-[22px] shrink-0 self-center text-soft" fill="currentColor" strokeWidth={0} />}
          <h1 id="all-games-title" className="font-display text-[28px] leading-[1.1]">
            {favorites ? "Favorites" : launcher ? launcherTitle(launcher) : "All games"}
          </h1>
          <span className="text-sm text-faint tabular-nums">{sorted.length}</span>
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="relative">
            <Search
              aria-hidden="true"
              className="pointer-events-none absolute top-1/2 left-3 size-3.5 -translate-y-1/2 text-faint"
              strokeWidth={1.8}
            />
            <input
              ref={search}
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape" && query) {
                  event.stopPropagation();
                  setQuery("");
                }
              }}
              placeholder="Find a game"
              aria-label="Find a game"
              className="h-[34px] w-[200px] rounded-lg border border-hairline bg-transparent pr-8 pl-8 text-[13px] text-text transition-colors outline-none placeholder:text-faint focus-visible:border-violet focus-visible:ring-2 focus-visible:ring-violet/30 [&::-webkit-search-cancel-button]:hidden"
            />
            {query && (
              <button
                type="button"
                onClick={() => {
                  setQuery("");
                  search.current?.focus();
                }}
                aria-label="Clear"
                className="absolute top-1/2 right-2 flex size-5 -translate-y-1/2 items-center justify-center rounded-full text-faint hover:text-text"
              >
                <X className="size-3.5" strokeWidth={1.8} />
              </button>
            )}
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
      {launcher && (
        <p className="mt-2 max-w-[600px] font-prose text-[17px] leading-[1.45] text-soft">
          <PhraseText phrase={launcherSentence(summaries, launcher)} />
        </p>
      )}
      <div className="mt-5 flex flex-wrap items-center justify-between gap-3">
        {statuses.length > 0 && (
          <div role="group" aria-label="Show games by status" className="segmented">
            {(["all", ...statuses] as const).map((key) => (
              <button
                key={key}
                type="button"
                aria-pressed={shownFilter === key}
                onClick={() => setFilter(key)}
                className="segment"
              >
                {key !== "all" && <GameStatusIcon status={key} className="size-3" />}
                {key === "all" ? "All" : GAME_STATUS_LABELS[key]}
                <span
                  className={cn(
                    "text-[11.5px] font-normal tabular-nums",
                    shownFilter === key ? "text-soft" : "text-faint",
                  )}
                >
                  {countOf(key)}
                </span>
              </button>
            ))}
          </div>
        )}
        <div className="ml-auto flex items-center gap-2">
          <DropdownMenu>
            <DropdownMenuTrigger
              aria-label={`Group games, now ${shownGroup === "none" ? "not grouped" : `by ${GAME_GROUPS[shownGroup].toLowerCase()}`}`}
              className="flex h-[34px] items-center gap-1.5 rounded-lg px-3 text-[12.5px] text-soft ring-1 ring-hairline transition-colors ring-inset hover:bg-glint/5 focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none data-popup-open:bg-glint/5"
            >
              <Rows3 className="size-3.5 text-faint" strokeWidth={1.8} />
              <span className={cn("font-medium", shownGroup === "none" ? "text-soft" : "text-text")}>
                {shownGroup === "none" ? "Group" : GAME_GROUPS[shownGroup]}
              </span>
              <ChevronDown className="size-3.5 text-faint" strokeWidth={1.8} />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-auto min-w-40">
              {groupings.map((option) => (
                <DropdownMenuItem key={option} onClick={() => changeGroup(option)}>
                  {option === "none" ? "No groups" : GAME_GROUPS[option]}
                  {option === shownGroup && <Check className="ml-auto size-4 text-violet" />}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          <DropdownMenu>
            <DropdownMenuTrigger
              aria-label={`Sort games, now ${GAME_SORTS[sort]}`}
              className="flex h-[34px] items-center gap-1.5 rounded-lg px-3 text-[12.5px] text-soft ring-1 ring-hairline transition-colors ring-inset hover:bg-glint/5 focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none data-popup-open:bg-glint/5"
            >
              <ArrowDownUp className="size-3.5 text-faint" strokeWidth={1.8} />
              <span className="font-medium text-text">{GAME_SORTS[sort]}</span>
              <ChevronDown className="size-3.5 text-faint" strokeWidth={1.8} />
            </DropdownMenuTrigger>
            <DropdownMenuContent align="end" className="w-auto min-w-40">
              {(Object.keys(GAME_SORTS) as GameSort[]).map((option) => (
                <DropdownMenuItem key={option} onClick={() => changeSort(option)}>
                  {GAME_SORTS[option]}
                  {option === sort && <Check className="ml-auto size-4 text-violet" />}
                </DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
        </div>
      </div>
      {summaries.length === 0 && (
        <p className="mt-6 max-w-[56ch] text-sm text-faint">
          {favorites
            ? "No favorites yet. The heart on a game's page or in the menu of its cover makes it one."
            : "No games yet. Discover the games your launchers installed, or add any game by hand."}
        </p>
      )}
      {sorted.length === 0 && query && (
        <p className="mt-6 text-sm text-faint">No game matches "{query.trim()}".</p>
      )}
      {groups.map(({ key, title, games }, index) => (
        <div key={key} className={title ? (index === 0 ? "mt-7" : "mt-10") : undefined}>
          {title && (
            <h2 className="flex items-baseline gap-2.5 font-display text-lg">
              {shownGroup === "launcher" && (
                <LauncherMark launcher={key} className="size-4 shrink-0 self-center text-soft" />
              )}
              {title}
              <span className="font-sans text-[13px] font-normal text-faint tabular-nums">{games.length}</span>
            </h2>
          )}
          <ul className={cn("grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))] gap-x-5 gap-y-7", title ? "mt-4" : "mt-6")}>
            {games.map((summary) => (
              <GameTile
                key={summary.game.id}
                summary={summary}
                playing={playing.has(summary.game.id)}
                onEdit={onEdit}
                onDelete={onDelete}
              />
            ))}
          </ul>
        </div>
      ))}
    </section>
  );
}

/** The hours a launcher's copies recorded and the game with most of them, with the time it counted before Vaultime apart. */
function launcherSentence(summaries: GameSummary[], launcher: string) {
  let trackedMs = 0;
  let earlierMs = 0;
  let top: { title: string; ms: number } | null = null;
  for (const summary of summaries) {
    const ms = summary.launcherMs[launcher] ?? 0;
    trackedMs += ms;
    if (summary.earlier?.source === launcher) earlierMs += summary.earlier.earlier_ms;
    if (ms > 0 && (!top || ms > top.ms)) top = { title: summary.game.title, ms };
  }
  return launcherPlaytime(launcherWhere(launcher), trackedMs, top, { launcher: launcherTitle(launcher), ms: earlierMs });
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
  const { game, cover, totalMs, lastPlayedAt, recentSuspiciousCount, recentRecoveredCount, status, favorite } = summary;
  const { setFavorite } = useLibrary();

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
          {(recentSuspiciousCount > 0 || recentRecoveredCount > 0) && (
            <Badge
              variant={recentSuspiciousCount > 0 ? "amber" : "sky"}
              className="absolute top-2 left-2 bg-ink/75 backdrop-blur-sm"
              title={
                recentSuspiciousCount > 0
                  ? `${plural(recentSuspiciousCount, "session")} with a clock jump or a record changed outside Vaultime in the last ${TRUST_BADGE_RECENT_DAYS} days`
                  : `${plural(recentRecoveredCount, "session")} rebuilt after an unclean exit in the last ${TRUST_BADGE_RECENT_DAYS} days`
              }
            >
              {recentSuspiciousCount > 0 ? "Suspicious" : "Recovered"}
            </Badge>
          )}
        </span>
        <span className="flex min-w-0 flex-col gap-0.5">
          <span className="truncate text-sm text-text">{game.title}</span>
          <span className="flex items-center gap-1.5 truncate text-xs text-faint">
            {favorite && (
              <span title="Favorite" className="flex shrink-0 items-center text-soft">
                <Heart className="size-3" fill="currentColor" strokeWidth={0} />
                <span className="sr-only">Favorite</span>
              </span>
            )}
            {status && (
              <span title={GAME_STATUS_LABELS[status]} className="flex shrink-0 items-center">
                <GameStatusIcon status={status} className="size-3" />
                <span className="sr-only">{GAME_STATUS_LABELS[status]}</span>
              </span>
            )}
            {playing ? (
              <>
                <span className="size-1.5 rounded-full bg-violet" />
                <span className="text-violet">Playing now</span>
              </>
            ) : totalMs > 0 || lastPlayedAt ? (
              // On a launcher's view the hours are only that launcher's, so a game played
              // through another copy still says when.
              <>
                {totalMs > 0 && <span className="tabular-nums">{formatHoursShort(totalMs)}</span>}
                {totalMs > 0 && lastPlayedAt && <span aria-hidden="true">·</span>}
                {lastPlayedAt && (
                  <span className="truncate">
                    <LastPlayed at={lastPlayedAt} />
                  </span>
                )}
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
          <DropdownMenuItem
            onClick={() => setFavorite(game.id, !favorite).catch(logFailure("changing a favorite from the library"))}
          >
            {favorite ? <HeartOff className="size-4" /> : <Heart className="size-4" />}
            {favorite ? "Remove from favorites" : "Add to favorites"}
          </DropdownMenuItem>
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
