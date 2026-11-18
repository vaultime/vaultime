// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ReactNode } from "react";
import { act, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { RunningStore, RunningStoreContext } from "@/features/library/library-context";
import { HOUR_MS, MINUTE_MS } from "@/lib/constants";
import { continuedAmounts } from "@/lib/session-lines";
import type { DayShare, Session } from "@/lib/types";
import { ContinuedAmounts, SessionCard } from "./DaySection";

/** A session that began late on the 17th and runs on past midnight. */
const evening = {
  id: "evening",
  game_id: "game",
  started_at_wall: new Date(2026, 10, 17, 23, 20).toISOString(),
  ended_at_wall: null,
  runtime_ms: HOUR_MS,
  active_ms: HOUR_MS,
  idle_ms: 0,
} as Session;

const share = (day: string, runtimeMs: number, fromBefore: boolean) =>
  ({ day, start_ms: 0, end_ms: 0, runtime_ms: runtimeMs, from_before: fromBefore, into_next: !fromBefore }) as DayShare;
const shares = [share("2026-11-17", 40 * MINUTE_MS, false), share("2026-11-18", 20 * MINUTE_MS, true)];

function running(store: RunningStore, children: ReactNode) {
  return render(<RunningStoreContext.Provider value={store}>{children}</RunningStoreContext.Provider>);
}

describe("a session past midnight in the journal", () => {
  it("shows in its card the whole of it as of the latest poll while it runs", () => {
    const store = new RunningStore();
    running(
      store,
      <SessionCard session={evening} shares={shares} day={new Date(2026, 10, 18)} title="Hades" mark="red" style={{}} />,
    );
    expect(screen.getByText("So far")).toBeTruthy();
    expect(screen.getByText("1h 00m")).toBeTruthy();

    act(() => store.set([{ ...evening, runtime_ms: 2 * HOUR_MS }]));
    expect(screen.getByText("2h 00m")).toBeTruthy();
    expect(screen.queryByText("1h 00m")).toBeNull();
  });

  it("counts its part after midnight against the whole as of the latest poll", () => {
    // Durations keep together with no-break spaces, which queries read as spaces.
    const shown = (wholeMs: number) => continuedAmounts(20 * MINUTE_MS, wholeMs, true).replace(/\s+/g, " ");
    const store = new RunningStore();
    running(store, <ContinuedAmounts share={shares[1]} stored={evening} />);
    expect(screen.getByText(shown(HOUR_MS))).toBeTruthy();

    act(() => store.set([{ ...evening, runtime_ms: 2 * HOUR_MS }]));
    expect(screen.getByText(shown(2 * HOUR_MS))).toBeTruthy();
  });
});
