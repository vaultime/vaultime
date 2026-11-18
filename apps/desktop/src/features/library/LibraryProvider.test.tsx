// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsContext } from "@/features/settings/settings-context";
import { ACTIVE_POLL_MS, LIBRARY_CHANGED_EVENT } from "@/lib/constants";
import { listGames, listSessions } from "@/lib/tauri";
import { LibraryProvider } from "./LibraryProvider";

/** The revision of the library the core reports. */
let revision = 1;
/** The sessions a list without the chain checks leaves unchecked. */
let unchecked: string[] = [];
const events = new Map<string, () => void>();

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(async (name: string, handler: () => void) => {
    events.set(name, handler);
    return () => events.delete(name);
  }),
}));
vi.mock("@/lib/tauri", () => ({
  getActiveSessions: vi.fn(async () => ({ sessions: [], played_today_ms: 0, counted_at_ms: {}, revision })),
  getLibraryRevision: vi.fn(async () => revision),
  listGames: vi.fn(async () => []),
  listSessions: vi.fn(async (_days: number, checked: boolean) => ({
    sessions: [],
    games: [],
    launchers: [],
    unchecked: checked ? [] : unchecked,
  })),
  listPreferredGameAssets: vi.fn(async () => []),
  listEarlierPlaytime: vi.fn(async () => []),
  listStatusChanges: vi.fn(async () => []),
  listSessionNotes: vi.fn(async () => []),
  listDevices: vi.fn(async () => []),
  listGameLinks: vi.fn(async () => []),
  getDeviceId: vi.fn(async () => "this-pc"),
}));

function start() {
  render(
    <SettingsContext.Provider
      value={{ values: {}, read: true, save: vi.fn(async () => {}), reload: vi.fn(async () => {}), reloads: 0 }}
    >
      <LibraryProvider>
        <span />
      </LibraryProvider>
    </SettingsContext.Provider>,
  );
}

/** Lets the timers and the core's answers run. */
async function wait(ms = 0) {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
}

function setVisibility(state: "visible" | "hidden") {
  Object.defineProperty(document, "visibilityState", { configurable: true, get: () => state });
  act(() => {
    document.dispatchEvent(new Event("visibilitychange"));
  });
}

describe("the library", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date(2026, 10, 17, 21, 0));
    revision = 1;
    unchecked = [];
    vi.mocked(listGames).mockClear();
    vi.mocked(listSessions).mockClear();
  });
  afterEach(() => {
    setVisibility("visible");
    vi.useRealTimers();
  });

  it("reloads only when the core changed it", async () => {
    start();
    await wait();
    expect(listGames).toHaveBeenCalledTimes(1);

    await wait(ACTIVE_POLL_MS * 3);
    expect(listGames).toHaveBeenCalledTimes(1);

    revision = 2;
    await wait(ACTIVE_POLL_MS);
    expect(listGames).toHaveBeenCalledTimes(2);

    // A window shown again with nothing changed shows what it had.
    setVisibility("hidden");
    await wait(ACTIVE_POLL_MS * 2);
    setVisibility("visible");
    await wait();
    expect(listGames).toHaveBeenCalledTimes(2);
  });

  it("takes a change event the poll already loaded as done", async () => {
    start();
    await wait();
    revision = 2;
    await wait(ACTIVE_POLL_MS);
    expect(listGames).toHaveBeenCalledTimes(2);

    // As when a game starts: the poll saw it, then the tray's event comes.
    await act(async () => events.get(LIBRARY_CHANGED_EVENT)?.());
    await wait();
    expect(listGames).toHaveBeenCalledTimes(2);

    revision = 3;
    await act(async () => events.get(LIBRARY_CHANGED_EVENT)?.());
    await wait();
    expect(listGames).toHaveBeenCalledTimes(3);
  });

  it("checks the chains a load left unchecked while hidden once the window shows", async () => {
    unchecked = ["session"];
    const checks = () => vi.mocked(listSessions).mock.calls.filter(([, checked]) => checked).length;
    start();
    await wait();
    expect(checks()).toBe(1);

    // A game ends while the window waits in the tray.
    setVisibility("hidden");
    revision = 2;
    await act(async () => events.get(LIBRARY_CHANGED_EVENT)?.());
    await wait();
    expect(listGames).toHaveBeenCalledTimes(2);
    expect(checks()).toBe(1);

    setVisibility("visible");
    await wait();
    expect(checks()).toBe(2);
  });

  it("tries a failed load again at the next poll", async () => {
    start();
    await wait();
    vi.mocked(listGames).mockRejectedValueOnce(new Error("busy"));
    revision = 2;
    await wait(ACTIVE_POLL_MS);
    expect(listGames).toHaveBeenCalledTimes(2);
    await wait(ACTIVE_POLL_MS);
    expect(listGames).toHaveBeenCalledTimes(3);
  });

  it("reloads when a day begins, as the recent counts move", async () => {
    start();
    await wait();
    vi.setSystemTime(new Date(2026, 10, 18, 0, 0, 30));
    await wait(ACTIVE_POLL_MS);
    expect(listGames).toHaveBeenCalledTimes(2);
  });
});
