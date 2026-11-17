// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ContextType } from "react";
import { act, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { AwayTimerContext } from "@/features/away/away-timer-context";
import { ActiveContext, CountedAtContext, LibraryContext } from "@/features/library/library-context";
import { MAX_TICK_GAP_MS, MINUTE_MS, SECOND_MS } from "@/lib/constants";
import { formatClock } from "@/lib/time";
import type { AwayTimer, Game, Session } from "@/lib/types";
import { at, session } from "@/test/sessions";
import { LiveBar } from "./LiveBar";

const NOW = new Date(2026, 10, 17, 21, 0);

const games = [
  { id: "launcher", title: "Launcher", executable_path: "C:/launcher.exe", metadata_json: '{"steps_aside":true}' },
  { id: "hades", title: "Hades", executable_path: "C:/hades.exe", metadata_json: "{}" },
] as Game[];

/** Hades, running for ten minutes as of the tracker's last count. */
const hades = session(at(2026, 11, 17, 20, 50), 10, { id: "hades-now", game_id: "hades", ended_at_wall: null });
/** The launcher Hades started from, whose clock stands still while Hades runs. */
const launcher = session(at(2026, 11, 17, 19, 0), 110, { id: "launcher-now", game_id: "launcher", ended_at_wall: null });

function show({
  active = [],
  countedAt = {},
  timer = null,
}: {
  active?: Session[];
  countedAt?: Record<string, number>;
  timer?: AwayTimer | null;
}) {
  const library = {
    games,
    covers: {},
    summaries: [],
    idleThresholdSeconds: 300,
    thisDeviceId: "desktop",
  } as unknown as NonNullable<ContextType<typeof LibraryContext>>;
  render(
    <MemoryRouter>
      <LibraryContext.Provider value={library}>
        <AwayTimerContext.Provider value={{ timer, start: async () => {}, stop: () => {} }}>
          <ActiveContext.Provider value={active}>
            <CountedAtContext.Provider value={countedAt}>
              <LiveBar />
            </CountedAtContext.Provider>
          </ActiveContext.Provider>
        </AwayTimerContext.Provider>
      </LibraryContext.Provider>
    </MemoryRouter>,
  );
}

/** Moves the clock on, with the ticks that fall in between. */
function wait(ms: number) {
  act(() => {
    vi.advanceTimersByTime(ms);
  });
}

function clocks(): string[] {
  return screen.getAllByRole("timer").map((clock) => clock.textContent ?? "");
}

describe("the live bar", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(NOW);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("runs the clock on from when the tracker counted the session being played", () => {
    show({
      active: [launcher, hades],
      countedAt: { [launcher.id]: NOW.getTime() - 50 * SECOND_MS, [hades.id]: NOW.getTime() - 20 * SECOND_MS },
    });
    expect(screen.getByText("Hades")).toBeTruthy();
    expect(clocks()).toEqual(["00:10:20"]);
    wait(5 * SECOND_MS);
    expect(clocks()).toEqual(["00:10:25"]);
  });

  it("stops at most MAX_TICK_GAP_MS past the last count", () => {
    show({ active: [hades], countedAt: { [hades.id]: NOW.getTime() } });
    wait(MAX_TICK_GAP_MS - SECOND_MS);
    expect(clocks()).toEqual([formatClock(hades.runtime_ms + MAX_TICK_GAP_MS - SECOND_MS)]);
    wait(10 * MINUTE_MS);
    expect(clocks()).toEqual([formatClock(hades.runtime_ms + MAX_TICK_GAP_MS)]);
  });

  it("runs the timer for play away from this PC without a cap", () => {
    show({ timer: { game_id: "hades", started_at: new Date(NOW.getTime() - 95.5 * MINUTE_MS).toISOString() } });
    expect(screen.getByText("Away from this PC since 19:24")).toBeTruthy();
    expect(clocks()).toEqual(["01:35:30"]);
    wait(10 * MINUTE_MS);
    expect(clocks()).toEqual(["01:45:30"]);
  });

  it("runs the away timer on beside a session whose clock stopped", () => {
    show({
      active: [hades],
      countedAt: { [hades.id]: NOW.getTime() },
      timer: { game_id: "launcher", started_at: NOW.toISOString() },
    });
    wait(10 * MINUTE_MS);
    expect(clocks()).toEqual([formatClock(hades.runtime_ms + MAX_TICK_GAP_MS), "00:10:00"]);
  });
});
