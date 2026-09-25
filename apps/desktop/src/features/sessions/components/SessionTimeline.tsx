// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Clock, Play } from "lucide-react";
import { Badge } from "@/components/ui/badge";
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
    <div className="flex items-center justify-between gap-4 rounded-2xl border border-border/70 bg-card/75 px-4 py-3">
      <div className="flex min-w-0 items-center gap-3">
        {isActive ? (
          <Play className="h-4 w-4 shrink-0 text-green-500" />
        ) : (
          <Clock className="h-4 w-4 shrink-0 text-muted-foreground" />
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
          <p className="text-sm tabular-nums text-foreground">
            Runtime {formatDuration(session.runtime_ms)}
          </p>
          <p className="text-xs tabular-nums text-muted-foreground">
            Active {formatCompactDuration(session.active_ms)}
            {" · "}
            Idle {formatCompactDuration(session.idle_ms)}
          </p>
        </div>
        <div className="flex flex-wrap items-center justify-end gap-2">
          {isActive && (
            <Badge
              variant="outline"
              className="border-green-500/50 text-green-500"
            >
              Live
            </Badge>
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
      <div className="flex h-48 flex-col items-center justify-center rounded-3xl border border-dashed border-border/70 bg-card/30">
        <Clock className="mb-3 h-10 w-10 text-muted-foreground/40" />
        <p className="text-sm text-muted-foreground">{emptyMessage}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      {groups.map((group) => (
        <section key={group.key} className="space-y-3">
          <div className="flex items-center justify-between gap-4">
            <div>
              <h3 className="text-sm font-semibold text-foreground">
                {group.label}
              </h3>
              <p className="text-xs text-muted-foreground">
                {group.sessions.length} session
                {group.sessions.length === 1 ? "" : "s"}
                {" · "}
                {formatCompactDuration(group.activeMs)} active
                {" · "}
                {formatCompactDuration(group.runtimeMs)} runtime
              </p>
            </div>
          </div>

          <div className="space-y-2">
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
