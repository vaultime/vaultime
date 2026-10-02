// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { NavLink, useLocation } from "react-router";
import {
  ArrowDownUp,
  BookOpen,
  ChartNoAxesColumn,
  Check,
  Cloud,
  LibraryBig,
  Search,
  SlidersHorizontal,
} from "lucide-react";
import { Logo } from "@/components/brand/Logo";
import { Cover } from "@/components/media/Cover";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { useLibrary } from "@/features/library/library-context";
import { COPYRIGHT_NOTICE, SETTING_KEYS } from "@/lib/constants";
import { GAME_SORTS, isGameSort, sortGames, type GameSort } from "@/lib/game-sort";
import * as api from "@/lib/tauri";
import { formatHoursShort, formatRelativeDay } from "@/lib/time";
import { cn } from "@/lib/utils";

const PAGES = [
  { to: "/library", label: "Library", icon: LibraryBig },
  { to: "/journal", label: "Journal", icon: BookOpen },
  { to: "/stats", label: "Stats", icon: ChartNoAxesColumn },
  { to: "/cloud", label: "Cloud", icon: Cloud },
  { to: "/settings", label: "Settings", icon: SlidersHorizontal },
] as const;

export function Rail({ onSearch }: { onSearch: () => void }) {
  const { visible: summaries, active } = useLibrary();
  const { session } = useCloudSession();
  const location = useLocation();
  const [appVersion, setAppVersion] = useState("");
  const [sort, setSort] = useState<GameSort>("recent");
  const playing = new Set(active.map((session) => session.game_id));
  const games = sortGames(summaries, sort);

  useEffect(() => {
    api.getAppVersion().then(setAppVersion).catch(() => {});
    api
      .listSettings()
      .then((settings) => {
        const saved = settings.find((setting) => setting.key === SETTING_KEYS.railSort)?.value;
        if (isGameSort(saved)) setSort(saved);
      })
      .catch(() => {});
  }, []);

  function changeSort(next: GameSort) {
    setSort(next);
    api.setSetting(SETTING_KEYS.railSort, next).catch(() => {});
  }

  return (
    <nav aria-label="Main" className="flex min-h-0 flex-col gap-6 border-r border-rule px-4 pt-7 pb-4 xl:px-5">
      <Logo className="justify-center" signedIn={session !== null} />

      <button
        type="button"
        onClick={onSearch}
        className="flex h-11 items-center gap-2.5 rounded-[10px] border border-hairline px-3 text-left text-sm text-faint transition-colors hover:border-hairline-strong hover:text-soft focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
      >
        <Search className="size-4 shrink-0" strokeWidth={1.6} />
        <span className="min-w-0 flex-1 truncate">Search</span>
        <kbd className="shrink-0 rounded-[5px] border border-hairline px-1.5 py-0.5 font-mono text-[11px]">Ctrl K</kbd>
      </button>

      <div className="flex flex-col gap-0.5">
        {PAGES.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            end={to !== "/library"}
            className={({ isActive }) =>
              cn(
                "flex h-11 items-center gap-3 rounded-[10px] px-3 text-[15px] font-medium transition-colors",
                isActive ? "bg-raised text-text" : "text-faint hover:bg-raised/60 hover:text-soft",
              )
            }
          >
            {({ isActive }) => (
              <>
                <Icon className="size-5" strokeWidth={1.6} />
                <span className="flex-1">{label}</span>
                {isActive && <span className="size-1.5 rounded-full bg-violet" />}
              </>
            )}
          </NavLink>
        ))}
      </div>

      <div className="flex min-h-0 flex-1 flex-col gap-1.5">
        <div className="flex items-center justify-between pr-1.5 pb-1 pl-3">
          <span className="flex items-baseline gap-2">
            <span className="label-caps">Your games</span>
            <span className="font-mono text-[11px] text-faint">{summaries.length}</span>
          </span>
          {summaries.length > 1 && (
            <DropdownMenu>
              <DropdownMenuTrigger
                aria-label={`Sort games, now ${GAME_SORTS[sort]}`}
                title="Sort games"
                className="flex h-7 items-center gap-1.5 rounded-md px-1.5 text-[11px] text-faint transition-colors hover:text-soft focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none data-popup-open:text-soft"
              >
                <ArrowDownUp className="size-3" strokeWidth={1.8} />
                {GAME_SORTS[sort]}
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
          )}
        </div>
        {summaries.length === 0 && (
          <p className="px-3 text-[13px] text-faint">Games you add show up here.</p>
        )}
        <ul className="no-scrollbar -mr-2 flex min-h-0 flex-col gap-0.5 overflow-y-auto pr-2">
          {games.map(({ game, cover, totalMs, lastPlayedAt }) => {
            const selected = location.pathname === `/library/${game.id}`;
            return (
              <li key={game.id}>
                <NavLink
                  to={`/library/${game.id}`}
                  className={cn(
                    "flex items-center gap-3 rounded-lg px-3 py-1.5 transition-colors",
                    selected ? "bg-raised" : "hover:bg-raised/60",
                  )}
                >
                  <Cover title={game.title} src={cover} variant="tile" className="size-9 text-lg" />
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-sm text-soft">{game.title}</span>
                    <span className="flex items-center gap-1.5 text-xs text-faint">
                      {playing.has(game.id) && <span className="size-1.5 rounded-full bg-violet" />}
                      {playing.has(game.id)
                        ? "Playing now"
                        : lastPlayedAt
                          ? formatRelativeDay(lastPlayedAt)
                          : "Not played yet"}
                    </span>
                  </span>
                  {totalMs > 0 && (
                    <span className="text-xs text-faint tabular-nums">{formatHoursShort(totalMs)}</span>
                  )}
                </NavLink>
              </li>
            );
          })}
        </ul>
      </div>

      <div className="flex flex-wrap items-center justify-center gap-x-2 gap-y-1 text-[11px] text-faint">
        {appVersion && <span className="font-mono">v{appVersion}</span>}
        {/* Only where both fit on one line, the narrow rail puts them on two. */}
        {appVersion && <span aria-hidden="true" className="hidden size-[3px] rounded-full bg-faint/60 xl:block" />}
        <span>{COPYRIGHT_NOTICE}</span>
      </div>
    </nav>
  );
}
