// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";

/** A number that goes up every `intervalMs` while `running`, to read live data again. */
export function useRefreshWhile(running: boolean, intervalMs: number): number {
  const [count, setCount] = useState(0);
  useEffect(() => {
    if (!running) return;
    const timer = setInterval(() => setCount((value) => value + 1), intervalMs);
    return () => clearInterval(timer);
  }, [running, intervalMs]);
  return count;
}
