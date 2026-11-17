// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ContextType } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { LibraryContext } from "@/features/library/library-context";
import { MINUTE_MS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import type { AwayTimer } from "@/lib/types";
import { useAwayTimer } from "./away-timer-context";
import { AwayTimerProvider } from "./AwayTimerProvider";

vi.mock("@tauri-apps/plugin-log", () => ({ warn: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({
  getAwayTimer: vi.fn(),
  startAwayTimer: vi.fn(),
  clearAwayTimer: vi.fn(async () => {}),
  addManualSession: vi.fn(async () => ({})),
}));

const refresh = vi.fn(async () => {});

function Controls() {
  const { timer, start, stop } = useAwayTimer();
  return (
    <>
      <p>{timer ? `Timing ${timer.game_id}` : "No timer"}</p>
      <button type="button" onClick={() => void start("hades")}>
        Start
      </button>
      <button type="button" onClick={stop}>
        Stop
      </button>
    </>
  );
}

function renderWith(timer: AwayTimer | null) {
  vi.mocked(api.getAwayTimer).mockResolvedValue(timer);
  const library = {
    loaded: true,
    games: [{ id: "hades", title: "Hades", launcher_source: null }],
    summaries: [],
    links: [],
    thisDeviceId: "desktop",
    refresh,
  } as unknown as NonNullable<ContextType<typeof LibraryContext>>;
  render(
    <LibraryContext.Provider value={library}>
      <AwayTimerProvider>
        <Controls />
      </AwayTimerProvider>
    </LibraryContext.Provider>,
  );
}

/** A timer for Hades that started 95 and a half minutes ago. */
function running(): AwayTimer {
  return { game_id: "hades", started_at: new Date(Date.now() - 95.5 * MINUTE_MS).toISOString() };
}

describe("the timer for play away from this PC", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("starts for a game", async () => {
    vi.mocked(api.startAwayTimer).mockResolvedValue({ game_id: "hades", started_at: new Date().toISOString() });
    renderWith(null);
    fireEvent.click(await screen.findByRole("button", { name: "Start" }));
    expect(await screen.findByText("Timing hades")).toBeTruthy();
    expect(api.startAwayTimer).toHaveBeenCalledWith("hades");
  });

  it("saves the timed play as a session when it stops", async () => {
    renderWith(running());
    await screen.findByText("Timing hades");
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(await screen.findByText("Save the timed play")).toBeTruthy();
    expect((screen.getByLabelText("Hours") as HTMLInputElement).value).toBe("1");
    expect((screen.getByLabelText("Minutes") as HTMLInputElement).value).toBe("35");
    fireEvent.click(screen.getByRole("button", { name: "Save session" }));
    await waitFor(() => expect(api.clearAwayTimer).toHaveBeenCalled());
    expect(api.addManualSession).toHaveBeenCalledWith("hades", expect.any(String), 95 * MINUTE_MS, "", null);
    expect(refresh).toHaveBeenCalled();
    expect(await screen.findByText("No timer")).toBeTruthy();
  });

  it("drops the timer of a game deleted since", async () => {
    vi.mocked(api.getAwayTimer)
      .mockResolvedValueOnce({ game_id: "gone", started_at: new Date().toISOString() })
      .mockResolvedValue(null);
    renderWith(null);
    await waitFor(() => expect(api.getAwayTimer).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("No timer")).toBeTruthy();
  });

  it("sets the timed play aside without a session", async () => {
    renderWith(running());
    await screen.findByText("Timing hades");
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    fireEvent.click(await screen.findByRole("button", { name: "Discard" }));
    expect(await screen.findByText("No timer")).toBeTruthy();
    expect(api.clearAwayTimer).toHaveBeenCalled();
    expect(api.addManualSession).not.toHaveBeenCalled();
  });
});
