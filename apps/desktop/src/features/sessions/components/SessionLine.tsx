// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { IntegrityBadge } from "@/components/status/IntegrityBadge";
import { describeSession, sessionAmounts, sessionTrustNote } from "@/lib/sentences";
import { formatSessionStart } from "@/lib/time";
import type { Session, SessionEvent } from "@/lib/types";
import { cn } from "@/lib/utils";

/** One session as a sentence, with its numbers and trust label below. */
export function SessionLine({
  session,
  events,
  gameTitle,
  when,
  bordered = true,
}: {
  session: Session;
  /** Events of this session or more, used to explain flags and skipped time. */
  events: SessionEvent[];
  /** Set when the list mixes games. */
  gameTitle?: string;
  /** Replaces the start day and time in the left column. */
  when?: string;
  bordered?: boolean;
}) {
  const note = sessionTrustNote(session, events);
  const live = !session.ended_at_wall;

  return (
    <article className={cn("flex items-baseline gap-5", bordered && "border-b border-rule py-3.5")}>
      <span className="w-[132px] shrink-0 font-mono text-[13px] text-faint">
        {when ?? formatSessionStart(session.started_at_wall)}
      </span>
      <div className="min-w-0 flex-1">
        <div className={cn("font-display text-[22px] leading-snug", live && "text-violet")}>
          {describeSession(session, gameTitle)}
        </div>
        <div className="mt-1 text-[13px] text-faint">
          {sessionAmounts(session)}
          {live ? " so far." : "."}
          {note && <span className="text-soft"> {note}</span>}
        </div>
      </div>
      <IntegrityBadge status={session.integrity_status} />
    </article>
  );
}
