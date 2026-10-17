// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ReactNode } from "react";
import { MINUTE_MS } from "@/lib/constants";
import { formatCalendarDay, formatHoursMinutes } from "@/lib/time";
import type { EarlierPlaytime } from "@/lib/types";
import { cn } from "@/lib/utils";

/** A titled part of the column beside the sessions. */
export function AsideSection({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section>
      <h2 className="label-caps mb-2.5">{title}</h2>
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


export function TotalRow({ label, value, accent = false }: { label: string; value: string; accent?: boolean }) {
  return (
    <div className="flex items-baseline justify-between border-b border-rule py-2.5">
      <span className="text-sm text-soft">{label}</span>
      <span className={cn("font-mono text-[15px] tabular-nums", accent && "text-violet")}>{value}</span>
    </div>
  );
}

