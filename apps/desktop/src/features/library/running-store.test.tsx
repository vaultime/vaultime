// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { act, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { Session } from "@/lib/types";
import { RunningStore, RunningStoreContext, useRunningSession } from "./library-context";

const draws = new Map<string, number>();

function Line({ id }: { id: string }) {
  const running = useRunningSession(id);
  draws.set(id, (draws.get(id) ?? 0) + 1);
  return <p>{running ? `${id} ${running.runtime_ms}` : `${id} ended`}</p>;
}

const session = (id: string, runtimeMs: number) => ({ id, runtime_ms: runtimeMs }) as Session;

describe("the running sessions", () => {
  it("draw again only the line of a session whose times moved", () => {
    const store = new RunningStore();
    render(
      <RunningStoreContext.Provider value={store}>
        <Line id="running" />
        <Line id="past" />
      </RunningStoreContext.Provider>,
    );
    expect(screen.getByText("running ended")).toBeTruthy();

    act(() => store.set([session("running", 1_000)]));
    act(() => store.set([session("running", 6_000)]));
    expect(screen.getByText("running 6000")).toBeTruthy();
    expect(draws.get("running")).toBe(3);
    expect(draws.get("past")).toBe(1);

    act(() => store.set([]));
    expect(screen.getByText("running ended")).toBeTruthy();
    expect(draws.get("past")).toBe(1);
  });
});
