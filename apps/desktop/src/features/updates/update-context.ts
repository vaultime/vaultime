// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";

export type UpdateStatus = "unknown" | "checking" | "current" | "available" | "installing" | "failed";

export interface UpdateState {
  status: UpdateStatus;
  /** The version ready to install, while one is. */
  version: string | null;
  /** Why the last check or install failed. */
  error: string | null;
  /** When the update server last answered. */
  checkedAt: Date | null;
  /** Looks for a new version now. */
  check: () => Promise<void>;
  /** Downloads the new version, installs it and restarts. */
  install: () => Promise<void>;
}

export const UpdateContext = createContext<UpdateState | null>(null);

export function useUpdates(): UpdateState {
  const value = useContext(UpdateContext);
  if (!value) throw new Error("useUpdates needs an UpdateProvider");
  return value;
}
