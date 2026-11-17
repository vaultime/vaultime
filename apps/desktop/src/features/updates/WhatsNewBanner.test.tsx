// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ContextType } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { LibraryContext } from "@/features/library/library-context";
import { SettingsContext } from "@/features/settings/settings-context";
import { SETTING_KEYS } from "@/lib/constants";
import type { Game } from "@/lib/types";
import { WhatsNewBanner } from "./WhatsNewBanner";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@/lib/tauri", () => ({ getAppVersion: vi.fn(async () => "0.7.0") }));
vi.mock("@/lib/whats-new", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@/lib/whats-new")>()),
  loadReleaseNotes: vi.fn(async (version: string) => ({
    version,
    date: "2026-12-01",
    groups: [
      { heading: "Library", notes: [{ title: "Seen running", text: "Games from your game folders show in Discover." }] },
      { heading: "Fixes", notes: [{ title: "", text: "One." }, { title: "", text: "Two." }] },
    ],
  })),
}));

const save = vi.fn(async () => {});

function renderBanner(values: Record<string, string>, games: Partial<Game>[]) {
  const library = { loaded: true, games, visible: [] } as unknown as NonNullable<ContextType<typeof LibraryContext>>;
  render(
    <SettingsContext.Provider value={{ values, read: true, save, reload: vi.fn(async () => {}), reloads: 0 }}>
      <LibraryContext.Provider value={library}>
        <WhatsNewBanner />
      </LibraryContext.Provider>
    </SettingsContext.Provider>,
  );
}

describe("what's new after an update", () => {
  beforeEach(() => {
    save.mockClear();
  });

  it("offers the release's changes once", async () => {
    renderBanner({ [SETTING_KEYS.whatsNewSeen]: "0.6.0" }, [{ id: "elden-ring" }]);
    fireEvent.click(await screen.findByRole("button", { name: /See what's new/ }));
    expect(await screen.findByText("What's new in Vaultime 0.7.0")).toBeTruthy();
    expect(screen.getByText("Seen running")).toBeTruthy();
    expect(screen.getByText("And 2 fixes, listed in the full changelog.")).toBeTruthy();
    expect(save).not.toHaveBeenCalled();
    // The footer button, after the cross in the corner.
    fireEvent.click(screen.getAllByRole("button", { name: "Close" }).at(-1)!);
    await waitFor(() => expect(save).toHaveBeenCalledWith(SETTING_KEYS.whatsNewSeen, "0.7.0"));
  });

  it("stays away on a fresh install and remembers the version", async () => {
    renderBanner({}, []);
    await waitFor(() => expect(save).toHaveBeenCalledWith(SETTING_KEYS.whatsNewSeen, "0.7.0"));
    expect(screen.queryByRole("button", { name: /See what's new/ })).toBeNull();
  });

  it("stays away once the version was seen", async () => {
    renderBanner({ [SETTING_KEYS.whatsNewSeen]: "0.7.0" }, [{ id: "elden-ring" }]);
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.queryByRole("button", { name: /See what's new/ })).toBeNull();
    expect(save).not.toHaveBeenCalled();
  });
});
