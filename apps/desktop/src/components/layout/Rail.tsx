// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useState } from "react";
import { NavLink, useLocation } from "react-router";
import { BookOpen, Cloud, LibraryBig, Search, SlidersHorizontal } from "lucide-react";
import { Logo } from "@/components/brand/Logo";
import { Cover } from "@/components/media/Cover";
import { useLibrary } from "@/features/library/library-context";
import * as api from "@/lib/tauri";
import { formatHoursShort, formatRelativeDay } from "@/lib/time";
import { cn } from "@/lib/utils";

const PAGES = [
  { to: "/library", label: "Library", icon: LibraryBig },
  { to: "/journal", label: "Journal", icon: BookOpen },
  { to: "/cloud", label: "Cloud", icon: Cloud },
  { to: "/settings", label: "Settings", icon: SlidersHorizontal },
] as const;

export function Rail({ onSearch }: { onSearch: () => void }) {
  const { summaries, active } = useLibrary();
  const location = useLocation();
  const [appVersion, setAppVersion] = useState("");
  const playing = new Set(active.map((session) => session.game_id));

  useEffect(() => {
    api.getAppVersion().then(setAppVersion).catch(() => {});
  }, []);

  return (
    <nav aria-label="Main" className="flex min-h-0 flex-col gap-6 border-r border-rule px-5 pt-7 pb-4">
      <Logo className="px-2" />

      <button
        type="button"
        onClick={onSearch}
        className="flex h-11 items-center gap-2.5 rounded-[10px] border border-hairline px-3 text-left text-sm text-faint transition-colors hover:border-hairline-strong hover:text-soft focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
      >
        <Search className="size-4 shrink-0" strokeWidth={1.6} />
        <span className="flex-1">Search or jump to</span>
        <kbd className="rounded-[5px] border border-hairline px-1.5 py-0.5 font-mono text-[11px]">Ctrl K</kbd>
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
        <div className="flex items-center justify-between px-3 pb-1.5">
          <span className="label-caps">Your games</span>
          <span className="font-mono text-[11px] text-faint">{summaries.length}</span>
        </div>
        {summaries.length === 0 && (
          <p className="px-3 text-[13px] text-faint">Games you add show up here.</p>
        )}
        <ul className="no-scrollbar -mr-2 flex min-h-0 flex-col gap-0.5 overflow-y-auto pr-2">
          {summaries.map(({ game, cover, runtimeMs, lastPlayedAt }) => {
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
                  {runtimeMs > 0 && (
                    <span className="text-xs text-faint tabular-nums">{formatHoursShort(runtimeMs)}</span>
                  )}
                </NavLink>
              </li>
            );
          })}
        </ul>
      </div>

      {appVersion && <span className="px-3 font-mono text-[11px] text-faint">v{appVersion}</span>}
    </nav>
  );
}
