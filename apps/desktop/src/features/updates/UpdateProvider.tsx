// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check as checkForUpdate } from "@tauri-apps/plugin-updater";
import { UPDATE_CHECK_INTERVAL_MS } from "@/lib/constants";
import { describeError } from "@/lib/utils";
import { UpdateContext, type UpdateStatus } from "./update-context";

/** Looks for new versions at start and every few hours, for the banner and the settings. */
export function UpdateProvider({ children }: { children: ReactNode }) {
  const [status, setStatus] = useState<UpdateStatus>("unknown");
  const [version, setVersion] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [checkedAt, setCheckedAt] = useState<Date | null>(null);
  const busy = useRef(false);
  // The player asked, so a failed check is worth saying.
  const asked = useRef(false);

  const look = useCallback(() => {
    if (busy.current) return Promise.resolve();
    busy.current = true;
    return checkForUpdate()
      .then((update) => {
        setCheckedAt(new Date());
        setVersion(update?.version ?? null);
        setStatus(update ? "available" : "current");
        setError(null);
        // Each answer holds a handle in the core. Installing asks again.
        void update?.close().catch(() => {});
      })
      .catch((checkError) => {
        // Offline or no update server. A check nobody asked for stays silent.
        if (!asked.current) return;
        setStatus("failed");
        setError(describeError(checkError));
      })
      .finally(() => {
        busy.current = false;
        asked.current = false;
      });
  }, []);

  useEffect(() => {
    void look();
    const timer = setInterval(() => void look(), UPDATE_CHECK_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [look]);

  const check = useCallback(async () => {
    asked.current = true;
    setStatus("checking");
    await look();
  }, [look]);

  const install = useCallback(async () => {
    if (busy.current) return;
    busy.current = true;
    setStatus("installing");
    setError(null);
    try {
      const update = await checkForUpdate();
      if (!update) {
        setVersion(null);
        setStatus("current");
        return;
      }
      await update.downloadAndInstall();
      await relaunch();
    } catch (installError) {
      setStatus("failed");
      setError(describeError(installError));
    } finally {
      busy.current = false;
    }
  }, []);

  const value = useMemo(
    () => ({ status, version, error, checkedAt, check, install }),
    [status, version, error, checkedAt, check, install],
  );
  return <UpdateContext.Provider value={value}>{children}</UpdateContext.Provider>;
}
