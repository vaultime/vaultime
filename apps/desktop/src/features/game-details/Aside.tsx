// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ReactNode } from "react";
import { MINUTE_MS } from "@/lib/constants";
import { markWords, nextMark, reachedMarks } from "@/lib/milestones";
import { formatCalendarDay, formatHoursMinutes } from "@/lib/time";
import type { EarlierPlaytime, Session } from "@/lib/types";
import { cn } from "@/lib/utils";
import { capitalize } from "@/lib/words";

/** A titled part of the column beside the sessions. */
export function AsideSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="label-small mb-2.5">{title}</h2>
      {children}
    </section>
  );
}


/** Playtime from before Vaultime, and what it is made of. */
export function EarlierSection({ earlier }: { earlier: EarlierPlaytime }) {
  const launcherMs = earlier.launcher_minutes * MINUTE_MS;
  return (
    <AsideSection title="Before Vaultime">
      <TotalRow label="From Steam" value={formatHoursMinutes(earlier.earlier_ms)} />
      {earlier.last_played_at && (
        <TotalRow label="Last played on Steam" value={formatCalendarDay(earlier.last_played_at)} />
      )}
      <p className="mt-2.5 text-[13px] leading-relaxed text-faint">
        Steam counted {formatHoursMinutes(launcherMs)} by {formatCalendarDay(earlier.imported_at)}
        {earlier.tracked_before_ms > 0
          ? `, of which ${formatHoursMinutes(Math.min(earlier.tracked_before_ms, launcherMs))} were tracked here too and count once`
          : ""}
        . It has no sessions, so the journal and the stats leave it out.
      </p>
    </AsideSection>
  );
}


/** The next hour mark as a ring, then the marks the game passed, each with the day it fell on. */
export function HourMarksSection({
  totalMs,
  sessions,
  earlierMs,
}: {
  totalMs: number;
  sessions: Session[];
  earlierMs?: number;
}) {
  const next = nextMark(totalMs);
  const reached = reachedMarks(sessions, earlierMs).reverse();
  const radius = 19;
  const around = 2 * Math.PI * radius;
  return (
    <AsideSection title="Hour marks">
      <div className="flex items-center gap-3.5 border-b border-rule pb-3.5">
        <svg viewBox="0 0 48 48" aria-hidden="true" className="size-11 shrink-0 -rotate-90">
          <circle cx="24" cy="24" r={radius} fill="none" strokeWidth="4" className="stroke-glint/10" />
          <circle
            cx="24"
            cy="24"
            r={radius}
            fill="none"
            strokeWidth="4"
            strokeLinecap="round"
            strokeDasharray={around}
            strokeDashoffset={around * (1 - next.progress)}
            className="stroke-violet"
          />
        </svg>
        <div className="min-w-0">
          <div className="text-[15px]">{capitalize(markWords(next.mark))}</div>
          <div className="text-[13px] text-faint tabular-nums">{formatHoursMinutes(next.leftMs)} to go</div>
        </div>
      </div>
      {reached.map(({ mark, session }) => (
        <div key={mark} className="flex items-baseline justify-between gap-4 border-b border-rule py-2.5">
          <span className="text-sm text-soft">{capitalize(markWords(mark))}</span>
          <span className="text-[13px] text-faint">
            {session ? formatCalendarDay(session.started_at_wall) : "Before Vaultime"}
          </span>
        </div>
      ))}
    </AsideSection>
  );
}

export function TotalRow({ label, value, accent = false }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="flex items-baseline justify-between border-b border-rule py-2.5">
      <span className="text-sm text-soft">{label}</span>
      <span className={cn("text-[15px] tabular-nums", accent && "text-violet")}>{value}</span>
    </div>
  );
}

