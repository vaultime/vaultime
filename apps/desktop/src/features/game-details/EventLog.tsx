// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { EVENT_LOG_LIMIT } from "@/lib/constants";
import { formatIntegrityEventType, getIntegrityEventDetail } from "@/lib/integrity";
import { compareTimes, formatSessionStart } from "@/lib/time";
import type { SessionEvent } from "@/lib/types";

/** The notable local events, for anyone who wants to check the record. */
export function EventLog({ events }: { events: SessionEvent[] }) {
  const notable = events.filter((event) => event.event_type !== "heartbeat");
  if (notable.length === 0) return null;
  const shown = [...notable]
    .sort((a, b) => compareTimes(b.event_time_wall, a.event_time_wall))
    .slice(0, EVENT_LOG_LIMIT);

  return (
    <details className="group border-t border-rule pt-4">
      <summary className="flex cursor-pointer list-none items-baseline justify-between text-sm text-faint hover:text-soft">
        <span>
          Event log, {notable.length} entr{notable.length === 1 ? "y" : "ies"}
        </span>
        <span className="text-xs group-open:hidden">Show</span>
        <span className="hidden text-xs group-open:inline">Hide</span>
      </summary>
      <p className="mt-3 text-[13px] text-faint">
        Every session keeps a hash-linked log on this PC. It can reveal edits to the record, but it cannot
        prove there were none.
      </p>
      <ul className="mt-2">
        {shown.map((event) => (
          <li key={event.id} className="flex items-baseline gap-5 border-b border-rule py-2.5 text-[13px]">
            <span className="w-[20ch] shrink-0 font-mono text-faint">{formatSessionStart(event.event_time_wall)}</span>
            <span className="w-40 shrink-0 text-soft">{formatIntegrityEventType(event.event_type)}</span>
            <span className="min-w-0 flex-1 text-faint">{getIntegrityEventDetail(event)}</span>
          </li>
        ))}
      </ul>
    </details>
  );
}

