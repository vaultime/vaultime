// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Cloud } from "lucide-react";

export function CloudPage() {
  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Cloud</h1>
        <p className="text-muted-foreground">
          Backup status, sync, and subscription management.
        </p>
      </div>

      <div className="flex h-64 flex-col items-center justify-center rounded-lg border border-dashed border-border">
        <Cloud className="mb-3 h-10 w-10 text-muted-foreground/50" />
        <p className="text-sm text-muted-foreground">
          Cloud sync is a premium feature. Available in a future release.
        </p>
      </div>
    </div>
  );
}
