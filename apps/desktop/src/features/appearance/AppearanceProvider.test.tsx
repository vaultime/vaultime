// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useSettings } from "@/features/settings/settings-context";
import { SettingsProvider } from "@/features/settings/SettingsProvider";
import { SETTING_KEYS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import { useAppearance } from "./appearance-context";
import { AppearanceProvider } from "./AppearanceProvider";

vi.mock("@tauri-apps/plugin-log", () => ({ warn: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({
  listSettings: vi.fn(),
  setSetting: vi.fn(async () => {}),
  getBackgroundImage: vi.fn(async () => null),
  setWindowLook: vi.fn(async () => true),
}));
vi.mock("@/lib/game-tint", async (original) => ({
  ...(await original<typeof import("@/lib/game-tint")>()),
  averageLightness: vi.fn(async () => 0.9),
}));

function Probe() {
  const { appearance } = useAppearance();
  const { reload } = useSettings();
  return (
    <button type="button" onClick={() => void reload()}>
      {appearance.accent}
    </button>
  );
}

/** Puts `cover` in front and shows what lies behind the pages. */
function BackdropProbe({ cover }: { cover: string }) {
  const { backdrop, backdropDim, showGame, change } = useAppearance();
  useEffect(() => showGame({ cover, colors: [] }), [cover, showGame]);
  return (
    <>
      <output aria-label="Backdrop">{backdrop ?? "none"}</output>
      <output aria-label="Dim">{backdropDim}</output>
      <button type="button" onClick={() => change({ source: "none" })}>
        None
      </button>
    </>
  );
}

describe("the backdrop", () => {
  it("shows the game in front, dims light art more and goes with None", async () => {
    vi.mocked(api.listSettings).mockResolvedValueOnce([]);
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <BackdropProbe cover="/covers/snowfield.jpg" />
        </AppearanceProvider>
      </SettingsProvider>,
    );
    const backdrop = await screen.findByLabelText("Backdrop");
    await waitFor(() => expect(backdrop.textContent).toBe("/covers/snowfield.jpg"));
    // 55 percent, and 20 more for art at 0.9, 0.4 above the bright line.
    await waitFor(() => expect(screen.getByLabelText("Dim").textContent).toBe("75"));
    fireEvent.click(screen.getByRole("button", { name: "None" }));
    expect(backdrop.textContent).toBe("none");
  });
});

describe("the look after a restore", () => {
  it("follows the settings the restore brought", async () => {
    vi.mocked(api.listSettings)
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([{ key: SETTING_KEYS.appearanceAccent, value: "teal", updated_at: "2026-10-20T08:00:00.000Z" }]);
    render(
      <SettingsProvider>
        <AppearanceProvider>
          <Probe />
        </AppearanceProvider>
      </SettingsProvider>,
    );

    fireEvent.click(await screen.findByRole("button", { name: "violet" }));
    expect(await screen.findByRole("button", { name: "teal" })).toBeDefined();
  });
});
