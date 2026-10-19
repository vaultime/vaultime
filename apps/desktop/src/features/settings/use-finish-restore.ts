// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback } from "react";
import { useLibrary } from "@/features/library/library-context";
import { useSettings } from "@/features/settings/settings-context";
import { continuesAs } from "@/lib/devices";
import type { PcTakeover } from "@/lib/types";
import { logFailure } from "@/lib/utils";

/**
 * What every restore does once the core replaced the history: read the
 * settings and the library again and say what happened. Returns the line to
 * show, "Cloud backup restored. This PC continues as Desktop."
 */
export function useFinishRestore() {
  const { refresh } = useLibrary();
  const { reload: reloadSettings } = useSettings();

  return useCallback(
    async (what: string, restored: { continues_as?: PcTakeover | null }) => {
      // A restore replaces the settings that are not kept per PC.
      await reloadSettings().catch(logFailure("reading the restored settings"));
      await refresh();
      return restored.continues_as ? `${what} restored. ${continuesAs(restored.continues_as)}` : `${what} restored.`;
    },
    [refresh, reloadSettings],
  );
}
