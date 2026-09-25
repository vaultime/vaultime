// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Clock, Loader2, Play } from "lucide-react";
import { useEffect, useState } from "react";
import type { Game, Session } from "@/lib/types";
import * as api from "@/lib/tauri";
import { useSessions, useActiveSessions } from "./useSessions";
import { Badge } from "@/components/ui/badge";

function formatDuration(ms: number): string {
  const totalSeconds = Math.floor(ms / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

function formatDate(iso: string): string {
  const date = new Date(iso + "Z");
  return date.toLocaleDateString(undefined, {
    weekday: "short",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function SessionRow({
  session,
  gameName,
}: {
  session: Session;
  gameName: string;
}) {
  const isActive = !session.ended_at_wall;

  return (
    <div className="flex items-center justify-between rounded-lg border border-border bg-card px-4 py-3">
      <div className="flex items-center gap-3">
        {isActive ? (
          <Play className="h-4 w-4 text-green-500" />
        ) : (
          <Clock className="h-4 w-4 text-muted-foreground" />
        )}
        <div>
          <p className="text-sm font-medium">{gameName}</p>
          <p className="text-xs text-muted-foreground">
            {formatDate(session.started_at_wall)}
          </p>
        </div>
      </div>

      <div className="flex items-center gap-3">
        <span className="text-sm tabular-nums text-muted-foreground">
          {isActive ? "In progress..." : formatDuration(session.runtime_ms)}
        </span>
        {isActive && (
          <Badge
            variant="outline"
            className="border-green-500/50 text-green-500"
          >
            Live
          </Badge>
        )}
        {!session.closed_cleanly && !isActive && (
          <Badge
            variant="outline"
            className="border-yellow-500/50 text-yellow-500"
          >
            {session.integrity_status}
          </Badge>
        )}
      </div>
    </div>
  );
}

export function SessionsPage() {
  const { sessions, loading, error, refresh } = useSessions();
  const { activeSessions } = useActiveSessions();
  const [gameMap, setGameMap] = useState<Record<string, string>>({});

  // Build a game name lookup from the game list.
  useEffect(() => {
    api.listGames().then((games: Game[]) => {
      const map: Record<string, string> = {};
      for (const g of games) {
        map[g.id] = g.title;
      }
      setGameMap(map);
    });
  }, [sessions]);

  // Auto-refresh when active sessions change.
  useEffect(() => {
    if (activeSessions.length > 0) {
      refresh();
    }
  }, [activeSessions, refresh]);

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  if (error) {
    return (
      <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4">
        <p className="text-sm text-destructive">{error}</p>
      </div>
    );
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Sessions</h1>
        <p className="text-muted-foreground">
          Timeline of your play sessions across all games.
        </p>
      </div>

      {sessions.length === 0 ? (
        <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed border-border">
          <Clock className="mb-3 h-10 w-10 text-muted-foreground/50" />
          <p className="text-sm text-muted-foreground">
            No sessions recorded yet. Start playing a tracked game.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          {sessions.map((session) => (
            <SessionRow
              key={session.id}
              session={session}
              gameName={gameMap[session.game_id] ?? "Unknown Game"}
            />
          ))}
        </div>
      )}
    </div>
  );
}
