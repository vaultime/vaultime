// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { useLibrary } from "@/features/library/library-context";
import { SEEN_GAMES_CHECK_MS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import { usePageVisible } from "@/lib/use-page-visible";

const SEEN_CHANGED_EVENT = "vaultime-seen-changed";

/** Tells the rail that the player added or dismissed a program seen running. */
export function seenGamesChanged() {
  window.dispatchEvent(new Event(SEEN_CHANGED_EVENT));
}

/**
 * How many programs seen running wait in Discover. Read while the window is
 * on screen: when it shows, every few minutes, when the library changes and
 * when the player adds or dismisses one.
 */
export function useSeenGamesCount(): number {
  const { games } = useLibrary();
  const visible = usePageVisible();
  const [count, setCount] = useState(0);

  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    const read = () => {
      api
        .listSeenGames()
        .then((seen) => {
          if (!cancelled) setCount(seen.length);
        })
        .catch(() => {});
    };
    read();
    const timer = window.setInterval(read, SEEN_GAMES_CHECK_MS);
    window.addEventListener(SEEN_CHANGED_EVENT, read);
    return () => {
      cancelled = true;
      window.clearInterval(timer);
      window.removeEventListener(SEEN_CHANGED_EVENT, read);
    };
  }, [games, visible]);

  return count;
}
