// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ReactNode } from "react";
import { Link, NavLink, useLocation } from "react-router";
import { BookOpen, ChartNoAxesColumn, Cloud, Heart, House, Layers, Plus, Settings2 } from "lucide-react";
import { LauncherMark } from "@/components/brand/LauncherMark";
import { Logo } from "@/components/brand/Logo";
import { Cover } from "@/components/media/Cover";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { LastPlayed } from "@/features/library/components/LastPlayed";
import { useCurrentSession, useLibrary, usePlaying } from "@/features/library/library-context";
import { useSeenGamesCount } from "@/features/library/use-seen-games";
import { RAIL_RECENT_GAMES } from "@/lib/constants";
import { launcherCounts, launcherTitle } from "@/lib/launchers";
import { cn } from "@/lib/utils";
import { plural } from "@/lib/words";

const PAGES = [
  { to: "/home", label: "Home", icon: House },
  { to: "/journal", label: "Journal", icon: BookOpen },
  { to: "/stats", label: "Stats", icon: ChartNoAxesColumn },
] as const;

const ROW =
  "flex items-center gap-2.5 rounded-[7px] px-2 text-[13px] font-medium transition-colors focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none";
const ROW_IDLE = "text-soft hover:bg-glint/5 hover:text-text";
const ROW_ON = "bg-glint/8 text-text";

const PAGE_ICON = "size-[17px]";
/** Filled marks look bigger than outlined icons of the same size. */
const MARK_ICON = "size-[15px]";

/** The pages, the library by launcher and the games played last, on the left of every page. */
export function Rail() {
  const { games, visible: summaries } = useLibrary();
  const seenGames = useSeenGamesCount();
  const playing = usePlaying();
  const currentSession = useCurrentSession();
  const current = currentSession ? (games.find((game) => game.id === currentSession.game_id) ?? null) : null;
  const { enabled: cloudEnabled } = useCloudSession();
  const location = useLocation();
  // The library comes most recently played first.
  const recent = summaries.filter((summary) => summary.lastPlayedAt).slice(0, RAIL_RECENT_GAMES);
  const launchers = launcherCounts(summaries);
  const inLibrary = location.pathname === "/library";
  const query = new URLSearchParams(location.search);
  const showingFavorites = inLibrary && query.get("favorites") === "1";
  const shownLauncher = inLibrary && !showingFavorites ? query.get("launcher") : null;

  return (
    <nav aria-label="Main" className="panel flex min-h-0 flex-col px-2.5 pt-3 pb-2">
      <Logo className="h-9 px-2" />

      <div className="mt-2 flex flex-col gap-px">
        {PAGES.map(({ to, label, icon: Icon }) => (
          <RailLink
            key={to}
            to={to}
            icon={<Icon className={PAGE_ICON} strokeWidth={1.75} />}
            active={location.pathname.startsWith(to)}
            className="h-[34px] text-[13.5px]"
          >
            {label}
          </RailLink>
        ))}
      </div>

      <RailRule />
      {/* Fades out at the foot, so rows cut off by a short window read as more to scroll. */}
      <div className="no-scrollbar -mx-1 flex min-h-0 flex-1 flex-col overflow-y-auto px-1 pb-3 [mask-image:linear-gradient(to_bottom,black_calc(100%_-_18px),transparent)]">
        <RailLabel
          action={
            <Link
              to="/library?discover=1"
              aria-label={seenGames > 0 ? `Discover games, ${plural(seenGames, "game")} seen running` : "Discover games"}
              title={seenGames > 0 ? `Discover games, ${plural(seenGames, "game")} seen running` : "Discover games"}
              className="relative flex size-6 items-center justify-center rounded-md text-faint transition-colors hover:bg-glint/5 hover:text-text focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
            >
              <Plus className="size-4" strokeWidth={1.8} />
              {seenGames > 0 && <span aria-hidden="true" className="absolute top-0.5 right-0.5 size-1.5 rounded-full bg-violet" />}
            </Link>
          }
        >
          Library
        </RailLabel>
        <div className="flex flex-col gap-px">
          <RailLink
            to="/library"
            icon={<Layers className={PAGE_ICON} strokeWidth={1.75} />}
            count={summaries.length}
            active={inLibrary && !shownLauncher && !showingFavorites}
          >
            All games
          </RailLink>
          <RailLink
            to="/library?favorites=1"
            icon={<Heart className={PAGE_ICON} strokeWidth={1.75} />}
            count={summaries.filter((summary) => summary.favorite).length}
            active={showingFavorites}
          >
            Favorites
          </RailLink>
          {launchers.map(({ launcher, count }) => (
            <RailLink
              key={launcher}
              to={`/library?launcher=${encodeURIComponent(launcher)}`}
              icon={<LauncherMark launcher={launcher} className={MARK_ICON} />}
              count={count}
              active={shownLauncher === launcher}
            >
              {launcherTitle(launcher)}
            </RailLink>
          ))}
        </div>

        {recent.length > 0 && (
          <>
            <RailRule />
            <RailLabel>Recently played</RailLabel>
            <ul className="flex flex-col gap-px">
              {recent.map(({ game, cover, lastPlayedAt }) => (
                <li key={game.id}>
                  <NavLink
                    to={`/library/${game.id}`}
                    className={({ isActive }) => cn(ROW, "h-11 font-normal", isActive ? ROW_ON : ROW_IDLE)}
                  >
                    <Cover
                      title={game.title}
                      src={cover}
                      variant="tile"
                      className="h-[38px] w-7 rounded-[4px] text-sm"
                    />
                    <span className="flex min-w-0 flex-col leading-[1.3]">
                      <span className="truncate font-medium">{game.title}</span>
                      {playing.has(game.id) ? (
                        <span className="truncate text-xs text-violet-hover">Playing now</span>
                      ) : (
                        <span className="truncate text-xs text-faint">
                          <LastPlayed at={lastPlayedAt} />
                        </span>
                      )}
                    </span>
                  </NavLink>
                </li>
              ))}
            </ul>
          </>
        )}
      </div>

      <div className="mt-auto flex items-center gap-0.5 border-t border-rule pt-2 pl-2 text-xs text-faint">
        <span
          aria-hidden="true"
          className={cn(
            "mr-2 size-[7px] shrink-0 rounded-full",
            current ? "bg-violet ring-3 ring-violet/20" : "bg-faint/70 ring-3 ring-faint/15",
          )}
        />
        <span className="min-w-0 flex-1 truncate">{current ? `Tracking ${current.title}` : "Watching for games"}</span>
        {cloudEnabled && (
          <FootLink to="/cloud" label="Cloud backup">
            <Cloud className="size-4" strokeWidth={1.7} />
          </FootLink>
        )}
        <FootLink to="/settings" label="Settings">
          <Settings2 className="size-4" strokeWidth={1.7} />
        </FootLink>
      </div>
    </nav>
  );
}

function RailLink({
  to,
  icon,
  count,
  active,
  className,
  children,
}: {
  to: string;
  /** Drawn in the current color, which follows the row. */
  icon: ReactNode;
  count?: number;
  /** Whether the row's page is open. NavLink would match the path alone, so every library row would count. */
  active: boolean;
  className?: string;
  children: ReactNode;
}) {
  return (
    <Link
      to={to}
      aria-current={active ? "page" : undefined}
      className={cn(ROW, "group h-8", active ? ROW_ON : ROW_IDLE, className)}
    >
      <span
        aria-hidden="true"
        className={cn(
          "flex size-[17px] shrink-0 items-center justify-center transition-colors group-hover:text-text",
          active ? "text-text" : "text-faint",
        )}
      >
        {icon}
      </span>
      <span className="min-w-0 flex-1 truncate">{children}</span>
      {count !== undefined && <span className="text-[11.5px] font-normal text-faint tabular-nums">{count}</span>}
    </Link>
  );
}

function RailLabel({ action, children }: { action?: ReactNode; children: ReactNode }) {
  return (
    <div className="flex h-[26px] items-center justify-between pr-1 pl-2 text-[11.5px] font-medium text-faint">
      {children}
      {action}
    </div>
  );
}

function RailRule() {
  return <div aria-hidden="true" className="mx-2 my-2.5 h-px shrink-0 bg-rule" />;
}

function FootLink({ to, label, children }: { to: string; label: string; children: ReactNode }) {
  return (
    <NavLink
      to={to}
      aria-label={label}
      title={label}
      className={({ isActive }) =>
        cn(
          "flex size-7 shrink-0 items-center justify-center rounded-[7px] transition-colors focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none",
          isActive ? "bg-glint/8 text-text" : "text-faint hover:bg-glint/5 hover:text-text",
        )
      }
    >
      {children}
    </NavLink>
  );
}
