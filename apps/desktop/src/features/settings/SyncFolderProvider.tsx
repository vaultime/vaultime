// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { warn } from "@tauri-apps/plugin-log";
import { SyncFolderContext } from "@/features/settings/sync-folder-context";
import { SYNC_FOLDER_CHECK_MS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import type { AutoMerge } from "@/lib/types";
import { describeError } from "@/lib/utils";

/**
 * Brings in the newer sync folder backups of the PCs merged here before, at
 * start and every hour, and keeps the ones that wait for the player. The
 * core tells the library when sessions came in.
 */
export function SyncFolderProvider({ children }: { children: ReactNode }) {
  const [waiting, setWaiting] = useState<AutoMerge[]>([]);

  useEffect(() => {
    let cancelled = false;
    const check = () => {
      api
        .mergeDueSyncBackups()
        .then((done) => {
          if (!cancelled) setWaiting(done.filter((merge) => merge.outcome !== "merged"));
        })
        // The next check tries again, a sync folder that is away comes back.
        .catch((error) => void warn(`automatic merge from the sync folder failed: ${describeError(error)}`));
    };
    const first = window.setTimeout(check, 0);
    const timer = window.setInterval(check, SYNC_FOLDER_CHECK_MS);
    return () => {
      cancelled = true;
      window.clearTimeout(first);
      window.clearInterval(timer);
    };
  }, []);

  const value = useMemo(() => ({ waiting }), [waiting]);
  return <SyncFolderContext.Provider value={value}>{children}</SyncFolderContext.Provider>;
}
