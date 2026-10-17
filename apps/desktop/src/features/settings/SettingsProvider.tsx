// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useState, type ReactNode } from "react";
import * as api from "@/lib/tauri";
import { SettingsContext } from "./settings-context";

/**
 * The stored settings, read once for the whole app and kept as they are
 * saved. The app draws once they are read, so no page changes a setting
 * after its first frame. The look of the last run is on the page already.
 */
export function SettingsProvider({ children }: { children: ReactNode }) {
  const [values, setValues] = useState<Record<string, string> | null>(null);

  useEffect(() => {
    let cancelled = false;
    api
      .listSettings()
      .then((settings) => {
        if (!cancelled) setValues(Object.fromEntries(settings.map((setting) => [setting.key, setting.value])));
      })
      .catch(() => {
        // The app works with the defaults when the settings cannot be read.
        if (!cancelled) setValues({});
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const save = useCallback(async (key: string, value: string) => {
    await api.setSetting(key, value);
    setValues((current) => ({ ...current, [key]: value }));
  }, []);

  const state = useMemo(() => (values ? { values, save } : null), [values, save]);
  if (!state) return null;
  return <SettingsContext.Provider value={state}>{children}</SettingsContext.Provider>;
}
