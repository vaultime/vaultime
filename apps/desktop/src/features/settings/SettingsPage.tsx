// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Settings } from "lucide-react";

export function SettingsPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
        <p className="text-muted-foreground">
          Tracking rules, thresholds, and preferences.
        </p>
      </div>

      <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed border-border">
        <Settings className="mb-3 h-10 w-10 text-muted-foreground/50" />
        <p className="text-sm text-muted-foreground">
          Settings will be available once tracking is implemented.
        </p>
      </div>
    </div>
  );
}
