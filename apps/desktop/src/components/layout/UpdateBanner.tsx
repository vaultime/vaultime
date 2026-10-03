// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import { Download, Loader2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useUpdates } from "@/features/updates/update-context";

export function UpdateBanner() {
  const { status, version, error, install } = useUpdates();
  // A newer version than the one dismissed shows the banner again.
  const [dismissed, setDismissed] = useState<string | null>(null);

  if (!version || dismissed === version) {
    return null;
  }

  const installing = status === "installing";
  return (
    <div className="flex items-center justify-between gap-4 border-b border-violet/30 bg-violet/10 px-8 py-2.5 xl:px-14">
      <p className="text-sm text-text">
        Vaultime <span className="font-semibold">v{version}</span> is
        available.
        {status === "failed" && error && <span className="ml-2 text-amber">The update failed: {error}</span>}
      </p>
      <div className="flex items-center gap-2">
        <Button size="sm" onClick={() => void install()} disabled={installing}>
          {installing ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Download className="h-3.5 w-3.5" />
          )}
          {installing ? "Installing" : "Update and restart"}
        </Button>
        <button
          type="button"
          onClick={() => setDismissed(version)}
          aria-label="Dismiss"
          className="rounded-full p-1.5 text-faint hover:bg-glint/10 hover:text-text"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
