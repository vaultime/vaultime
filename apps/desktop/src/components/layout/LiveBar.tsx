// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router";
import { ArrowUpRight, Gamepad2 } from "lucide-react";
import { Cover } from "@/components/media/Cover";
import { useLibrary } from "@/features/library/library-context";
import { getIntegrityMeta } from "@/lib/integrity";
import { LIVE_TICK_MS, SECONDS_PER_MINUTE } from "@/lib/constants";
import { formatClock, formatHoursMinutes, parseVaultimeDate, UI_LOCALE } from "@/lib/time";
import { usePageVisible } from "@/lib/use-page-visible";

/** The bar at the bottom of every page: the running game, like a music player. */
export function LiveBar() {
  const { active, activePolledAt, games, covers, sessions, idleThresholdSeconds } = useLibrary();
  const [now, setNow] = useState(() => Date.now());
  const current = active[0];
  const visible = usePageVisible();

  // Tick every second while something runs and the window is seen, so the
  // timer does not jump every poll.
  useEffect(() => {
    if (!current || !visible) return;
    const timer = setInterval(() => setNow(Date.now()), LIVE_TICK_MS);
    return () => clearInterval(timer);
  }, [current, visible]);

  const todayMs = useMemo(() => {
    const midnight = new Date();
    midnight.setHours(0, 0, 0, 0);
    return sessions
      .filter((session) => parseVaultimeDate(session.started_at_wall) >= midnight)
      .reduce((sum, session) => sum + session.runtime_ms, 0);
  }, [sessions]);

  if (!current) {
    const watched = games.filter((game) => game.executable_path).length;
    return (
      <footer
        aria-label="Live session"
        className="col-span-2 grid h-[76px] grid-cols-3 items-center border-t border-rule bg-bar px-6"
      >
        <div className="flex items-center gap-3.5">
          <span className="flex size-11 items-center justify-center rounded-md bg-raised text-faint">
            <Gamepad2 className="size-5" strokeWidth={1.6} />
          </span>
          <div className="flex flex-col gap-0.5">
            <span className="text-[15px] font-medium">Nothing running</span>
            <span className="text-[13px] text-faint">Start a game and it shows up here</span>
          </div>
        </div>
        <span className="text-center font-mono text-[13px] text-faint">
          Today {formatHoursMinutes(todayMs)}
        </span>
        <span className="text-right text-[13px] text-faint">
          Watching {watched} game{watched === 1 ? "" : "s"}
        </span>
      </footer>
    );
  }

  const game = games.find((g) => g.id === current.game_id);
  const title = game?.title ?? "Unknown game";
  const sincePoll = Math.max(0, now - activePolledAt);
  const runtimeMs = current.runtime_ms + sincePoll;
  const counted = current.active_ms + current.idle_ms;
  const activeShare = counted > 0 ? current.active_ms / counted : 1;
  const trust = getIntegrityMeta(current.integrity_status).label;
  const startedAt = parseVaultimeDate(current.started_at_wall).toLocaleTimeString(UI_LOCALE, {
    hour: "2-digit",
    minute: "2-digit",
  });

  return (
    <footer
      aria-label="Live session"
      className="col-span-2 grid h-[76px] grid-cols-3 items-center border-t border-rule bg-bar px-6"
    >
      <div className="flex min-w-0 items-center gap-3.5">
        <Cover title={title} src={covers[current.game_id]} variant="tile" className="size-11 text-xl" />
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="truncate text-[15px] font-medium">
            {title}
            {active.length > 1 && <span className="ml-2 text-[13px] text-faint">and {active.length - 1} more</span>}
          </span>
          <span className="flex items-center gap-2 text-[13px] text-faint">
            <span className="size-2 rounded-full bg-violet ring-3 ring-violet/25" />
            Tracking since {startedAt}, {trust}
          </span>
        </div>
      </div>

      <div className="flex flex-col items-center gap-1.5">
        <span className="font-mono text-[22px] tracking-wide tabular-nums" role="timer" aria-label={`Running for ${formatHoursMinutes(runtimeMs)}`}>
          {formatClock(runtimeMs)}
        </span>
        <div className="flex items-center gap-2.5 text-xs text-faint tabular-nums">
          <span className="whitespace-nowrap">
            <span className="label-caps mr-1.5">Active</span>
            {formatHoursMinutes(current.active_ms)}
          </span>
          <span aria-hidden="true" className="flex h-1.5 w-20 gap-0.5 lg:w-36 xl:w-56">
            <span className="rounded-full bg-violet" style={{ width: `${Math.round(activeShare * 100)}%` }} />
            <span className="flex-1 rounded-full bg-idle" />
          </span>
          <span className="whitespace-nowrap">
            <span className="label-caps mr-1.5">Idle</span>
            {formatHoursMinutes(current.idle_ms)}
          </span>
        </div>
      </div>

      <div className="flex items-center justify-end gap-4">
        <span className="hidden text-[13px] text-faint xl:inline">
          Counts as idle after {Math.round(idleThresholdSeconds / SECONDS_PER_MINUTE)} min away
        </span>
        <Link
          to={`/library/${current.game_id}`}
          aria-label={`Open ${title}`}
          className="flex size-10 items-center justify-center rounded-full border border-hairline text-soft transition-colors hover:bg-raised hover:text-text"
        >
          <ArrowUpRight className="size-4" strokeWidth={1.8} />
        </Link>
      </div>
    </footer>
  );
}
