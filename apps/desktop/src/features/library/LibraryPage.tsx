// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useMemo, useState } from "react";
import { Gamepad2, Loader2 } from "lucide-react";
import { useGames } from "./useGames";
import { useActiveSessions } from "../sessions/useSessions";
import { AddGameDialog } from "./components/AddGameDialog";
import { EditGameDialog } from "./components/EditGameDialog";
import { DeleteGameDialog } from "./components/DeleteGameDialog";
import { GameCard } from "./components/GameCard";
import type { Game, Session } from "@/lib/types";
import * as api from "@/lib/tauri";

export function LibraryPage() {
  const { games, loading, error, refresh } = useGames();
  const { activeSessions } = useActiveSessions();
  const [editingGame, setEditingGame] = useState<Game | null>(null);
  const [deletingGame, setDeletingGame] = useState<Game | null>(null);
  const [allSessions, setAllSessions] = useState<Session[]>([]);

  // Fetch all sessions for playtime totals.
  useEffect(() => {
    api.listSessions().then(setAllSessions).catch(() => {});
  }, [games, activeSessions]);

  // Build a set of currently running game IDs.
  const runningGameIds = useMemo(() => {
    const ids = new Set<string>();
    for (const s of activeSessions) {
      ids.add(s.game_id);
    }
    return ids;
  }, [activeSessions]);

  // Build total playtime per game from closed sessions.
  const playtimeByGame = useMemo(() => {
    const map: Record<string, number> = {};
    for (const s of allSessions) {
      if (s.ended_at_wall) {
        map[s.game_id] = (map[s.game_id] ?? 0) + s.runtime_ms;
      }
    }
    return map;
  }, [allSessions]);

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">Library</h1>
          <p className="text-muted-foreground">
            {games.length > 0
              ? `${games.length} game${games.length === 1 ? "" : "s"} tracked`
              : "Your game collection and playtime at a glance."}
          </p>
        </div>
        <AddGameDialog onAdded={refresh} />
      </div>

      {loading && (
        <div className="flex h-64 items-center justify-center">
          <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
        </div>
      )}

      {error && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
          {error}
        </div>
      )}

      {!loading && !error && games.length === 0 && (
        <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed border-border">
          <Gamepad2 className="mb-3 h-10 w-10 text-muted-foreground/50" />
          <p className="text-sm text-muted-foreground">
            No games added yet. Click &quot;Add Game&quot; to get started.
          </p>
        </div>
      )}

      {!loading && games.length > 0 && (
        <div className="grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5">
          {games.map((game) => (
            <GameCard
              key={game.id}
              game={game}
              isRunning={runningGameIds.has(game.id)}
              totalPlaytimeMs={playtimeByGame[game.id] ?? 0}
              onEdit={setEditingGame}
              onDelete={setDeletingGame}
            />
          ))}
        </div>
      )}

      <EditGameDialog
        game={editingGame}
        onClose={() => setEditingGame(null)}
        onSaved={refresh}
      />
      <DeleteGameDialog
        game={deletingGame}
        onClose={() => setDeletingGame(null)}
        onDeleted={refresh}
      />
    </div>
  );
}
