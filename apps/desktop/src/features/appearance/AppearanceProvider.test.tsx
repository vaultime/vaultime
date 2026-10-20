// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen } from "@testing-library/react";
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

function Probe() {
  const { appearance } = useAppearance();
  const { reload } = useSettings();
  return (
    <button type="button" onClick={() => void reload()}>
      {appearance.accent}
    </button>
  );
}

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
