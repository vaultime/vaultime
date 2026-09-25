// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Gamepad2 } from "lucide-react";

export function LibraryPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Library</h1>
        <p className="text-muted-foreground">
          Your game collection and playtime at a glance.
        </p>
      </div>

      <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed border-border">
        <Gamepad2 className="mb-3 h-10 w-10 text-muted-foreground/50" />
        <p className="text-sm text-muted-foreground">
          No games added yet. Add your first game to get started.
        </p>
      </div>
    </div>
  );
}
