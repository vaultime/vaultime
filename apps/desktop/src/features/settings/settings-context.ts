// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";

export interface SettingsState {
  /** The stored settings by key, as read last and saved since. */
  values: Record<string, string>;
  /** False when reading them failed and the defaults stand in. */
  read: boolean;
  /** Saves a setting and keeps its value for everything that reads it. */
  save: (key: string, value: string) => Promise<void>;
  /** Reads the settings again, after a restore replaced those that are not kept per PC. */
  reload: () => Promise<void>;
}

export const SettingsContext = createContext<SettingsState | null>(null);

export function useSettings(): SettingsState {
  const value = useContext(SettingsContext);
  if (!value) throw new Error("useSettings needs a SettingsProvider");
  return value;
}
