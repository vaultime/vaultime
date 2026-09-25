// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Clock } from "lucide-react";

export function SessionsPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Sessions</h1>
        <p className="text-muted-foreground">
          Timeline of your play sessions across all games.
        </p>
      </div>

      <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed border-border">
        <Clock className="mb-3 h-10 w-10 text-muted-foreground/50" />
        <p className="text-sm text-muted-foreground">
          No sessions recorded yet. Start playing a tracked game.
        </p>
      </div>
    </div>
  );
}
