// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ContextType } from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { LibraryContext } from "@/features/library/library-context";
import { SettingsContext } from "@/features/settings/settings-context";
import { SETTING_KEYS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import type { FolderPc } from "@/lib/types";
import { SyncFolderSection } from "./SyncFolderSection";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => "D:\\Dropbox") }));
vi.mock("@/lib/tauri", () => ({
  getSyncFolder: vi.fn(),
  setSyncFolder: vi.fn(),
  listSyncFolderPcs: vi.fn(),
}));

const FOLDER = "D:\\Dropbox\\Vaultime";

function pc(deviceId: string, name: string, changes: Partial<FolderPc> = {}): FolderPc {
  return {
    device_id: deviceId,
    name,
    backup_id: `${deviceId}-backup`,
    backup_created_at: "2026-11-16T20:00:00.000Z",
    sessions_count: 12,
    this_pc: false,
    merged_before: false,
    newer: true,
    ...changes,
  };
}

/** The section on a PC with the library and settings around it. */
function renderSection(values: Record<string, string> = {}) {
  const library = { sessions: [], refresh: vi.fn(async () => {}), thisDeviceId: "desktop" } as unknown as NonNullable<
    ContextType<typeof LibraryContext>
  >;
  render(
    <SettingsContext.Provider value={{ values, read: true, save: vi.fn(async () => {}), reload: vi.fn(async () => {}), reloads: 0 }}>
      <LibraryContext.Provider value={library}>
        <SyncFolderSection onError={vi.fn()} />
      </LibraryContext.Provider>
    </SettingsContext.Provider>,
  );
}

describe("the sync folder in Settings", () => {
  beforeEach(() => {
    vi.mocked(api.getSyncFolder).mockReset();
    vi.mocked(api.setSyncFolder).mockReset();
    vi.mocked(api.listSyncFolderPcs).mockReset();
  });

  it("copies a backup into the folder the player chooses and lists the PCs there", async () => {
    vi.mocked(api.getSyncFolder).mockResolvedValue(null);
    vi.mocked(api.listSyncFolderPcs)
      .mockResolvedValueOnce([])
      .mockResolvedValue([pc("desktop", "Desktop", { this_pc: true }), pc("laptop", "Laptop")]);
    vi.mocked(api.setSyncFolder).mockResolvedValue(FOLDER);
    renderSection();

    fireEvent.click(await screen.findByRole("button", { name: /Choose/ }));
    expect(await screen.findByText(FOLDER)).toBeTruthy();
    expect(api.setSyncFolder).toHaveBeenCalledWith("D:\\Dropbox");
    expect(screen.getByText("Desktop, this PC")).toBeTruthy();
    expect(screen.getByText(/After the first merge, newer backups come in by themselves/)).toBeTruthy();
    expect(screen.getByText(/Choose the same folder on your other PCs/)).toBeTruthy();
  });

  it("says no new backup reaches the folder while automatic backups are off", async () => {
    vi.mocked(api.getSyncFolder).mockResolvedValue(FOLDER);
    vi.mocked(api.listSyncFolderPcs).mockResolvedValue([]);
    renderSection({ [SETTING_KEYS.autoBackup]: "false" });

    expect(await screen.findByText(/Automatic backups are off/)).toBeTruthy();
    expect(screen.getByText("No backups in the folder yet.")).toBeTruthy();
  });

  it("stops using the folder and leaves its backups there", async () => {
    vi.mocked(api.getSyncFolder).mockResolvedValue(FOLDER);
    vi.mocked(api.listSyncFolderPcs).mockResolvedValue([pc("laptop", "Laptop", { merged_before: true, newer: false })]);
    vi.mocked(api.setSyncFolder).mockResolvedValue(null);
    renderSection();

    expect(await screen.findByText(/Merged\./)).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(await screen.findByText(/The backups stay in the folder/)).toBeTruthy();
    expect(api.setSyncFolder).toHaveBeenCalledWith(null);
    expect(screen.getByText("Not set")).toBeTruthy();
  });
});
