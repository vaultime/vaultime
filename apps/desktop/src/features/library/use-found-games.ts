// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { FOUND_GAMES_EVENT } from "@/lib/constants";
import * as api from "@/lib/tauri";
import type { NewGames } from "@/lib/types";
import { usePageVisible } from "@/lib/use-page-visible";

/**
 * The games the core found at start, once the window is on screen and no
 * other dialog is open. A window that started in the tray offers them the
 * next time it is shown. Offered games are remembered right away, so they
 * come once.
 */
export function useFoundGames(otherDialogOpen: boolean): { found: NewGames | null; dismiss: () => void } {
  const visible = usePageVisible();
  const [found, setFound] = useState<NewGames | null>(null);
  // Counts the core's reports, each one asks again.
  const [reports, setReports] = useState(0);
  const blocked = useRef(otherDialogOpen);
  useEffect(() => {
    blocked.current = otherDialogOpen;
  }, [otherDialogOpen]);

  useEffect(() => {
    const stop = listen(FOUND_GAMES_EVENT, () => setReports((count) => count + 1));
    return () => {
      stop.then((unlisten) => unlisten()).catch(() => {});
    };
  }, []);

  useEffect(() => {
    if (!visible || otherDialogOpen || found) return;
    let current = true;
    void (async () => {
      try {
        const next = await api.listFoundGames();
        if (!current || blocked.current || next.games.length === 0) return;
        await api.rememberOfferedGames(next.games.map((game) => game.executable_path));
        setFound(next);
      } catch (error) {
        console.warn("found games not offered", error);
      }
    })();
    return () => {
      current = false;
    };
  }, [visible, otherDialogOpen, found, reports]);

  const dismiss = useCallback(() => setFound(null), []);
  return { found, dismiss };
}
