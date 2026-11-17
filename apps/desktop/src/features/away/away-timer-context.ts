// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";
import type { AwayTimer } from "@/lib/types";

export interface AwayTimerState {
  /**
   * The timer running for play away from this PC, null while none runs. A
   * game linked since it started names the game it counts as.
   */
  timer: AwayTimer | null;
  /** Starts timing a game. */
  start: (gameId: string) => Promise<void>;
  /** Asks the player to save the timed play, which stops the timer. */
  stop: () => void;
}

export const AwayTimerContext = createContext<AwayTimerState>({
  timer: null,
  start: async () => {},
  stop: () => {},
});

export function useAwayTimer(): AwayTimerState {
  return useContext(AwayTimerContext);
}
