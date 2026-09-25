// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Clock, Play } from "lucide-react";
import { IntegrityBadge } from "@/components/status/IntegrityBadge";
import { formatCompactDuration, formatDuration, formatSessionDate } from "@/lib/time";
import { groupSessionsByDay } from "@/lib/session-stats";
import type { Session } from "@/lib/types";

interface SessionTimelineProps {
  sessions: Session[];
  gameMap?: Record<string, string>;
  showGameName?: boolean;
  emptyMessage?: string;
}

function SessionRow({
  session,
  gameName,
  showGameName,
}: {
  session: Session;
  gameName?: string;
  showGameName: boolean;
}) {
  const isActive = !session.ended_at_wall;

  return (
    <div
      className={`flex items-center justify-between gap-4 rounded-2xl border px-4 py-3 transition-colors ${
        isActive
          ? "border-green-500/30 bg-green-500/[0.04]"
          : "border-border/70 bg-card/75 hover:bg-card"
      }`}
    >
      <div className="flex min-w-0 items-center gap-3">
        {isActive ? (
          <div className="relative flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-green-500/15">
            <Play className="h-3.5 w-3.5 text-green-400" />
            <span className="absolute -top-0.5 -right-0.5 flex h-2.5 w-2.5">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-green-400 opacity-75" />
              <span className="relative inline-flex h-2.5 w-2.5 rounded-full bg-green-400" />
            </span>
          </div>
        ) : (
          <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-xl bg-white/[0.05]">
            <Clock className="h-3.5 w-3.5 text-muted-foreground" />
          </div>
        )}
        <div className="min-w-0">
          {showGameName && (
            <p className="truncate text-sm font-medium">
              {gameName ?? "Unknown Game"}
            </p>
          )}
          <p className="text-xs text-muted-foreground">
            {formatSessionDate(session.started_at_wall)}
          </p>
        </div>
      </div>

      <div className="flex shrink-0 items-center gap-3">
        <div className="text-right">
          <p className="text-sm font-semibold tabular-nums text-foreground">
            {formatDuration(session.runtime_ms)}
          </p>
          <p className="text-[11px] tabular-nums text-muted-foreground">
            {formatCompactDuration(session.active_ms)} active
          </p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {isActive && (
            <span className="rounded-full border border-green-500/40 bg-green-500/10 px-2 py-0.5 text-[10px] font-bold uppercase tracking-wider text-green-400">
              Live
            </span>
          )}
          <IntegrityBadge status={session.integrity_status} />
        </div>
      </div>
    </div>
  );
}

export function SessionTimeline({
  sessions,
  gameMap = {},
  showGameName = true,
  emptyMessage = "No sessions recorded yet.",
}: SessionTimelineProps) {
  const groups = groupSessionsByDay(sessions);

  if (groups.length === 0) {
    return (
      <div className="flex h-48 flex-col items-center justify-center rounded-3xl border border-dashed border-white/10 bg-white/[0.02]">
        <div className="rounded-2xl bg-white/[0.04] p-3">
          <Clock className="h-8 w-8 text-muted-foreground/40" />
        </div>
        <p className="mt-3 text-sm text-muted-foreground">{emptyMessage}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {groups.map((group) => (
        <section key={group.key} className="space-y-2">
          <div className="flex items-center justify-between gap-4 px-1">
            <h3 className="text-sm font-bold text-foreground">
              {group.label}
            </h3>
            <p className="text-[11px] tabular-nums text-muted-foreground">
              {group.sessions.length} session
              {group.sessions.length === 1 ? "" : "s"}
              {" · "}
              {formatCompactDuration(group.activeMs)} active
              {" · "}
              {formatCompactDuration(group.runtimeMs)} runtime
            </p>
          </div>

          <div className="space-y-1.5">
            {group.sessions.map((session) => (
              <SessionRow
                key={session.id}
                session={session}
                gameName={gameMap[session.game_id]}
                showGameName={showGameName}
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
