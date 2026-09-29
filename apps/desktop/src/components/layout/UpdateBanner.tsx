// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

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
        if (!cancelled && update) {
          setUpdateVersion(update.version);
        }
      } catch {
        // Offline or no update endpoint. Not worth surfacing.
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
      if (update) {
        await update.downloadAndInstall();
        await relaunch();
      } else {
        setInstalling(false);
      }
    } catch {
      setInstalling(false);
    }
  }

  return (
    <div className="flex items-center justify-between gap-4 border-b border-violet/30 bg-violet/10 px-8 py-2.5 xl:px-14">
      <p className="text-sm text-text">
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
          {installing ? "Installing" : "Update and restart"}
        </Button>
        <button
          type="button"
          onClick={() => setDismissed(true)}
          aria-label="Dismiss"
          className="rounded-full p-1.5 text-faint hover:bg-white/10 hover:text-text"
        >
          <X className="h-4 w-4" />
        </button>
      </div>
    </div>
  );
}
