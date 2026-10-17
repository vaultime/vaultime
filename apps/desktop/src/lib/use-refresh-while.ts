// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useRef, useState } from "react";
import { usePageVisible } from "@/lib/use-page-visible";

/**
 * A number that goes up every `intervalMs` while `running` and the window is
 * seen, and once when the window shows again while `running`, to read live
 * data again.
 */
export function useRefreshWhile(running: boolean, intervalMs: number): number {
  const visible = usePageVisible();
  const [count, setCount] = useState(0);
  const wasHidden = useRef(false);
  useEffect(() => {
    if (!visible) {
      wasHidden.current = true;
      return;
    }
    const showsAgain = wasHidden.current;
    wasHidden.current = false;
    if (!running) return;
    // A timer of no length, so the count changes outside the effect.
    const again = showsAgain ? setTimeout(() => setCount((value) => value + 1)) : undefined;
    const timer = setInterval(() => setCount((value) => value + 1), intervalMs);
    return () => {
      clearTimeout(again);
      clearInterval(timer);
    };
  }, [running, visible, intervalMs]);
  return count;
}
