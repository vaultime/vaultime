// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ComponentType, ReactNode } from "react";
import { Link, NavLink, useLocation } from "react-router";
import { BookOpen, ChartNoAxesColumn, Cloud, House, Layers, Plus, Settings2 } from "lucide-react";
import { Logo } from "@/components/brand/Logo";
import { Cover } from "@/components/media/Cover";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { LastPlayed } from "@/features/library/components/LastPlayed";
import { useCurrentSession, useLibrary, usePlaying } from "@/features/library/library-context";
import { RAIL_RECENT_GAMES } from "@/lib/constants";
import { cn } from "@/lib/utils";

const PAGES = [
  { to: "/home", label: "Home", icon: House },
  { to: "/journal", label: "Journal", icon: BookOpen },
  { to: "/stats", label: "Stats", icon: ChartNoAxesColumn },
] as const;

const ROW =
  "flex items-center gap-2.5 rounded-[7px] px-2 text-[13px] font-medium transition-colors focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none";
const ROW_IDLE = "text-soft hover:bg-glint/5 hover:text-text";
const ROW_ON = "bg-glint/8 text-text";

/** The pages, the library and the games played last, on the left of every page. */
export function Rail() {
  const { games, visible: summaries } = useLibrary();
  const playing = usePlaying();
  const currentSession = useCurrentSession();
  const current = currentSession ? (games.find((game) => game.id === currentSession.game_id) ?? null) : null;
  const { enabled: cloudEnabled, session } = useCloudSession();
  const location = useLocation();
  // The library comes most recently played first.
  const recent = summaries.filter((summary) => summary.lastPlayedAt).slice(0, RAIL_RECENT_GAMES);

  return (
    <nav aria-label="Main" className="panel flex min-h-0 flex-col px-2.5 pt-3 pb-2">
      <Logo small className="h-9 px-2" signedIn={session !== null} />

      <div className="mt-2 flex flex-col gap-px">
        {PAGES.map(({ to, label, icon }) => (
          <RailLink key={to} to={to} icon={icon} className="h-[34px] text-[13.5px]">
            {label}
          </RailLink>
        ))}
      </div>

      <RailRule />
      <RailLabel
        action={
          <Link
            to="/library?discover=1"
            aria-label="Discover games"
            title="Discover games"
            className="flex size-6 items-center justify-center rounded-md text-faint transition-colors hover:bg-glint/5 hover:text-text focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
          >
            <Plus className="size-4" strokeWidth={1.8} />
          </Link>
        }
      >
        Library
      </RailLabel>
      <div className="flex flex-col gap-px">
        <RailLink to="/library" icon={Layers} count={summaries.length} active={location.pathname === "/library"}>
          All games
        </RailLink>
      </div>

      {recent.length > 0 && (
        <>
          <RailRule />
          <RailLabel>Recently played</RailLabel>
          <ul className="no-scrollbar flex min-h-0 flex-col gap-px overflow-y-auto">
            {recent.map(({ game, cover, lastPlayedAt }) => (
              <li key={game.id}>
                <NavLink
                  to={`/library/${game.id}`}
                  className={({ isActive }) => cn(ROW, "h-11 font-normal", isActive ? ROW_ON : ROW_IDLE)}
                >
                  <Cover title={game.title} src={cover} variant="tile" className="h-[38px] w-7 rounded-[4px] text-sm" />
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
  icon: Icon,
  count,
  active,
  className,
  children,
}: {
  to: string;
  icon: ComponentType<{ className?: string; strokeWidth?: number }>;
  count?: number;
  /** For rows whose page also has a query, which NavLink alone would not tell apart. */
  active?: boolean;
  className?: string;
  children: ReactNode;
}) {
  return (
    <NavLink to={to} end className={({ isActive }) => cn(ROW, "group h-8", (active ?? isActive) ? ROW_ON : ROW_IDLE, className)}>
      {({ isActive }) => (
        <>
          <Icon
            className={cn(
              "size-[17px] shrink-0 transition-colors group-hover:text-text",
              (active ?? isActive) ? "text-text" : "text-faint",
            )}
            strokeWidth={1.75}
          />
          <span className="min-w-0 flex-1 truncate">{children}</span>
          {count !== undefined && <span className="text-[11.5px] font-normal text-faint tabular-nums">{count}</span>}
        </>
      )}
    </NavLink>
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
