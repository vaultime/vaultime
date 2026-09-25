// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useState } from "react";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { Download, Loader2, X } from "lucide-react";
import { Button } from "@/components/ui/button";

export function UpdateBanner() {
  const [updateVersion, setUpdateVersion] = useState<string | null>(null);
  const [installing, setInstalling] = useState(false);
  const [dismissed, setDismissed] = useState(false);

  useEffect(() => {
    let cancelled = false;

    async function checkForUpdate() {
      try {
        const update = await check();
        if (!cancelled && update?.available) {
          setUpdateVersion(update.version);
        }
      } catch {
        // Silently ignore update check failures (offline, no endpoint configured, etc.)
      }
    }

    void checkForUpdate();

    return () => {
      cancelled = true;
    };
  }, []);

  if (!updateVersion || dismissed) {
    return null;
  }

  async function handleInstall() {
    try {
      setInstalling(true);
      const update = await check();
      if (update?.available) {
        await update.downloadAndInstall();
        await relaunch();
      }
    } catch {
      setInstalling(false);
    }
  }

  return (
    <div className="flex items-center justify-between gap-4 rounded-2xl border border-primary/30 bg-primary/10 px-4 py-3">
      <p className="text-sm text-foreground">
        Vaultime <span className="font-semibold">v{updateVersion}</span> is
        available.
      </p>
      <div className="flex items-center gap-2">
        <Button size="sm" onClick={handleInstall} disabled={installing}>
          {installing ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Download className="h-3.5 w-3.5" />
          )}
          {installing ? "Installing..." : "Update & Restart"}
        </Button>
        <button
          type="button"
          onClick={() => setDismissed(true)}
          className="rounded-lg p-1.5 text-muted-foreground hover:bg-white/10 hover:text-foreground"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
