// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";
import type { AutoMerge } from "@/lib/types";

export interface SyncFolderState {
  /** The sync folder backups the last automatic merge left for the player, with the reason. */
  waiting: AutoMerge[];
}

export const SyncFolderContext = createContext<SyncFolderState>({ waiting: [] });

export function useSyncFolder(): SyncFolderState {
  return useContext(SyncFolderContext);
}
