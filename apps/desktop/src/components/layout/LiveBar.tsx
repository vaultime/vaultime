// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { Link } from "react-router";
import { ArrowUpRight, Gamepad2 } from "lucide-react";
import { Cover } from "@/components/media/Cover";
import {
  useActiveSessions,
  useCurrentSession,
  useLastPoll,
  useLibrary,
  usePlayedToday,
} from "@/features/library/library-context";
import { getIntegrityMeta } from "@/lib/integrity";
import { LIVE_TICK_MS, SECONDS_PER_MINUTE } from "@/lib/constants";
import { formatClock, formatClockTime, formatHoursMinutes, parseVaultimeDate } from "@/lib/time";
import { usePageVisible } from "@/lib/use-page-visible";
import { cn } from "@/lib/utils";

/** The panel at the bottom of every page: the running game, like a music player. */
export function LiveBar() {
  const { games, covers, summaries, idleThresholdSeconds } = useLibrary();
  const active = useActiveSessions();
  const activePolledAt = useLastPoll();
  const current = useCurrentSession();

  // Counted by the core at each poll, as the journal counts the day.
  const todayMs = usePlayedToday();

  if (!current) {
    const watched = games.filter((game) => game.executable_path).length;
    return (
      <footer aria-label="Live session" className={BAR}>
        <div className={SIDE}>
          <span className="flex h-12 w-[34px] shrink-0 items-center justify-center rounded-[5px] text-faint ring-1 ring-hairline ring-inset">
            <Gamepad2 className="size-4" strokeWidth={1.6} />
          </span>
          <div className="flex min-w-0 flex-col leading-[1.35]">
            <span className="truncate text-[13.5px] font-medium">Nothing running</span>
            <span className="truncate text-xs text-faint">Start a game and it shows up here</span>
          </div>
        </div>
        <span className="flex-1 text-center text-[12.5px] text-faint">
          Today <span className="font-medium text-text tabular-nums">{formatHoursMinutes(todayMs)}</span>
        </span>
        <span className={cn(SIDE, "justify-end text-xs text-faint")}>
          Watching {watched} game{watched === 1 ? "" : "s"}
        </span>
      </footer>
    );
  }

  const game = games.find((g) => g.id === current.game_id);
  const title = game?.title ?? "Unknown game";
  // Two installs of one game running at once are one game here.
  const others = new Set(active.map((session) => session.game_id).filter((id) => id !== current.game_id)).size;
  const counted = current.active_ms + current.idle_ms;
  const activeShare = counted > 0 ? current.active_ms / counted : 1;
  const trust = getIntegrityMeta(current.integrity_status).label;
  const startedAt = formatClockTime(parseVaultimeDate(current.started_at_wall));
  const tint = summaries.find((summary) => summary.game.id === current.game_id)?.tint;

  return (
    <footer
      aria-label="Live session"
      className={BAR}
      // The colors of the game being played wash in from its cover.
      style={tint ? { backgroundImage: `linear-gradient(90deg, color-mix(in oklab, ${tint.fill} 70%, transparent), transparent 40%)` } : undefined}
    >
      <div className={SIDE}>
        <Cover title={title} src={covers[current.game_id]} variant="tile" className="h-12 w-[34px] rounded-[5px] text-lg" />
        <div className="flex min-w-0 flex-col leading-[1.35]">
          <span className="truncate text-[13.5px] font-medium">
            {title}
            {others > 0 && <span className="ml-2 text-xs font-normal text-faint">and {others} more</span>}
          </span>
          <span className="flex items-center gap-2 text-xs text-faint">
            <span className="size-1.5 shrink-0 rounded-full bg-violet ring-3 ring-violet/25" />
            <span className="truncate">
              Since {startedAt}
              <span aria-hidden="true" className="mx-1.5 opacity-60">
                ·
              </span>
              {trust}
            </span>
          </span>
        </div>
      </div>

      <div className="flex min-w-0 flex-1 flex-col items-center gap-1.5">
        <RunningClock runtimeMs={current.runtime_ms} polledAt={activePolledAt} />
        <div className="flex items-center gap-2.5 text-[11.5px] text-faint tabular-nums">
          <span className="whitespace-nowrap">Active {formatHoursMinutes(current.active_ms)}</span>
          <span aria-hidden="true" className="block h-1 w-20 overflow-hidden rounded-full bg-idle lg:w-40 xl:w-60">
            <span className="block h-full rounded-full bg-violet" style={{ width: `${Math.round(activeShare * 100)}%` }} />
          </span>
          <span className="whitespace-nowrap">Idle {formatHoursMinutes(current.idle_ms)}</span>
        </div>
      </div>

      <div className={cn(SIDE, "justify-end gap-2.5 text-xs text-faint")}>
        <span className="hidden truncate xl:inline">
          Idle after {Math.round(idleThresholdSeconds / SECONDS_PER_MINUTE)} min away
        </span>
        <Link
          to={`/library/${current.game_id}`}
          aria-label={`Open ${title}`}
          title={`Open ${title}`}
          className="flex size-8 shrink-0 items-center justify-center rounded-lg text-soft transition-colors hover:bg-raised hover:text-text focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
        >
          <ArrowUpRight className="size-4" strokeWidth={1.8} />
        </Link>
      </div>
    </footer>
  );
}

const BAR = "panel col-span-2 flex h-[72px] items-center gap-[18px] pr-3.5 pl-3";
/** The two ends of the bar, of equal width so the clock sits in the middle. */
const SIDE = "flex w-[230px] min-w-0 shrink-0 items-center gap-3 xl:w-[290px]";

/**
 * The running time, ticking every second from the last poll while the window
 * is seen, so it does not jump every poll. Only the clock draws each second.
 */
function RunningClock({ runtimeMs, polledAt }: { runtimeMs: number; polledAt: number }) {
  const [now, setNow] = useState(() => Date.now());
  const visible = usePageVisible();
  useEffect(() => {
    if (!visible) return;
    const timer = setInterval(() => setNow(Date.now()), LIVE_TICK_MS);
    return () => clearInterval(timer);
  }, [visible]);
  const shownMs = runtimeMs + Math.max(0, now - polledAt);
  return (
    <span className="font-mono text-[19px] leading-none font-medium tracking-wide tabular-nums" role="timer" aria-label={`Running for ${formatHoursMinutes(shownMs)}`}>
      {formatClock(shownMs)}
    </span>
  );
}
