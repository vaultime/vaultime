// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ContextType } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { LibraryContext } from "@/features/library/library-context";
import { SettingsContext } from "@/features/settings/settings-context";
import * as api from "@/lib/tauri";
import type { LocalBackupSummary } from "@/lib/types";
import { LocalBackupsSection } from "./LocalBackupsSection";

vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(async () => "D:\\Backups\\Laptop") }));
vi.mock("@tauri-apps/plugin-log", () => ({ warn: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({
  inspectLocalBackup: vi.fn(),
  importLocalBackup: vi.fn(),
  listBackupSnapshots: vi.fn(async () => []),
}));

const BACKUP_PATH = "D:\\Backups\\Laptop\\vaultime-backup-20261003T120000Z";

function backup(continuesAs: LocalBackupSummary["continues_as"], restored = false): LocalBackupSummary {
  return {
    backup_id: "laptop-backup",
    backup_version: 1,
    created_at: "2026-10-03T12:00:00.000Z",
    app_version: "0.5.0",
    source_device_id: "laptop",
    schema_migrations: [],
    overall_checksum: "checksum",
    games_count: 7,
    sessions_count: 52,
    assets_count: 7,
    asset_file_count: 7,
    backup_path: BACKUP_PATH,
    manifest_path: `${BACKUP_PATH}\\manifest.json`,
    restart_required: restored,
    continues_as: continuesAs,
  };
}

/** The section on a PC with the library and settings around it. */
function renderSection() {
  const refresh = vi.fn(async () => {});
  const reload = vi.fn(async () => {});
  const library = { refresh, thisDeviceId: "this-pc" } as unknown as NonNullable<ContextType<typeof LibraryContext>>;
  render(
    <SettingsContext.Provider value={{ values: {}, read: true, save: vi.fn(async () => {}), reload, reloads: 0 }}>
      <LibraryContext.Provider value={library}>
        <LocalBackupsSection
          initial={{ autoBackup: true, folder: "C:\\Backups", snapshots: [] }}
          start={false}
          onError={vi.fn()}
          onRestartRequired={vi.fn()}
        />
      </LibraryContext.Provider>
    </SettingsContext.Provider>,
  );
  return { refresh, reload };
}

async function restore() {
  fireEvent.click(screen.getByRole("button", { name: /Restore from a backup/ }));
  fireEvent.click(await screen.findByRole("button", { name: /Replace with this backup/ }));
}

describe("restoring a local backup", () => {
  beforeEach(() => {
    vi.mocked(api.importLocalBackup).mockReset();
    vi.mocked(api.inspectLocalBackup).mockReset();
  });

  it("takes over the backup's PC when the preview offered it, then reads everything again", async () => {
    const laptop = { device_id: "laptop", name: "Laptop" };
    vi.mocked(api.inspectLocalBackup).mockResolvedValue(backup(laptop));
    vi.mocked(api.importLocalBackup).mockResolvedValue(backup(laptop, true));
    const { refresh, reload } = renderSection();

    await restore();
    await screen.findByText("Backup restored. This PC continues as Laptop.");
    expect(api.importLocalBackup).toHaveBeenCalledWith(BACKUP_PATH, true);
    expect(reload).toHaveBeenCalled();
    expect(refresh).toHaveBeenCalled();
  });

  it("never takes over when the preview did not offer it", async () => {
    vi.mocked(api.inspectLocalBackup).mockResolvedValue(backup(null));
    vi.mocked(api.importLocalBackup).mockResolvedValue(backup(null, true));
    const { refresh } = renderSection();

    await restore();
    await screen.findByText("Backup restored.");
    expect(api.importLocalBackup).toHaveBeenCalledWith(BACKUP_PATH, false);
    await waitFor(() => expect(refresh).toHaveBeenCalled());
  });
});
