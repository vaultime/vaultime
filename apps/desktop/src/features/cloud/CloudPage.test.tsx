// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ContextType } from "react";
import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { LibraryContext } from "@/features/library/library-context";
import { SettingsContext } from "@/features/settings/settings-context";
import { getThisPc } from "@/lib/tauri";
import type { CloudAuthSession, CloudBackupRecord, CloudBackupRestoreResult, ThisPc } from "@/lib/types";
import { CloudSessionContext, type CloudSessionContextValue } from "./cloud-context";
import { CloudPage } from "./CloudPage";

vi.mock("@tauri-apps/plugin-log", () => ({ warn: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({ getThisPc: vi.fn() }));

const SESSION = {
  access_token: "access",
  refresh_token: "refresh",
  expires_at: "2099-01-01T00:00:00.000Z",
  refresh_expires_at: "2099-01-01T00:00:00.000Z",
  user: { id: "account-1", email: "player@example.com", role: "user" },
} as unknown as CloudAuthSession;

/** A backup on the server, made on `source` or on a PC it does not name. */
function cloudBackup(source: string | null): CloudBackupRecord {
  return {
    id: "backup-1",
    label: "Before the reinstall",
    storage_key: "key",
    checksum: "checksum",
    size_bytes: 1_000,
    backup_created_at: "2026-10-18T10:00:00.000Z",
    uploaded_at: "2026-10-18T10:00:00.000Z",
    status: "complete",
    client_device_id: null,
    metadata_json: source
      ? ({ source_device_id: source, games_count: 7, sessions_count: 38 } as CloudBackupRecord["metadata_json"])
      : null,
  };
}

function restored(continuesAs: { device_id: string; name: string | null } | null): CloudBackupRestoreResult {
  return {
    backup: cloudBackup("laptop"),
    restored_summary: {
      created_at: "2026-10-18T10:00:00.000Z",
      source_device_id: "laptop",
      overall_checksum: "checksum",
      games_count: 7,
      sessions_count: 38,
      assets_count: 7,
      asset_file_count: 7,
      restart_required: true,
      continues_as: continuesAs,
    },
  };
}

/** The Cloud page of a signed in PC whose server holds `backup`. */
function renderPage(backup: CloudBackupRecord, result: CloudBackupRestoreResult) {
  const refresh = vi.fn(async () => {});
  const restoreRemoteBackup = vi.fn(async () => result);
  const cloud = {
    enabled: true,
    apiBaseUrl: "https://vaultime.example",
    initializing: false,
    session: SESSION,
    device: null,
    deviceError: null,
    connectionError: null,
    backupKeyReady: true,
    autoBackup: true,
    autoBackupError: null,
    isAdmin: false,
    setAutoBackup: vi.fn(),
    listBackups: vi.fn(async () => [backup]),
    getStorage: vi.fn(async () => ({ backup_bytes: 0, artwork_bytes: 0, limit_bytes: 1 })),
    restoreRemoteBackup,
  } as unknown as CloudSessionContextValue;
  const library = { refresh, thisDeviceId: "this-pc" } as unknown as NonNullable<ContextType<typeof LibraryContext>>;
  render(
    <SettingsContext.Provider value={{ values: {}, read: true, save: vi.fn(async () => {}), reload: vi.fn(async () => {}) }}>
      <LibraryContext.Provider value={library}>
        <CloudSessionContext.Provider value={cloud}>
          <CloudPage />
        </CloudSessionContext.Provider>
      </LibraryContext.Provider>
    </SettingsContext.Provider>,
  );
  return { refresh, restoreRemoteBackup };
}

function thisPc(fresh: boolean): ThisPc {
  return { device_id: "this-pc", name: "Desk", fresh, ledger: {} as ThisPc["ledger"] };
}

/** Opens the restore dialog of the only backup and confirms it. Returns the dialog's text. */
async function restoreTheBackup(): Promise<string> {
  fireEvent.click(await screen.findByRole("button", { name: "Restore" }));
  const dialog = await screen.findByRole("dialog");
  const replace = within(dialog).getByRole<HTMLButtonElement>("button", { name: /Replace with this backup/ });
  await waitFor(() => expect(replace.disabled).toBe(false));
  const text = dialog.textContent ?? "";
  fireEvent.click(replace);
  return text;
}

describe("restoring a cloud backup", () => {
  beforeEach(() => {
    vi.mocked(getThisPc).mockReset();
    window.localStorage.clear();
  });

  it("lets a fresh install take over the PC that made the backup", async () => {
    vi.mocked(getThisPc).mockResolvedValue(thisPc(true));
    const { refresh, restoreRemoteBackup } = renderPage(
      cloudBackup("laptop"),
      restored({ device_id: "laptop", name: "Laptop" }),
    );

    const dialog = await restoreTheBackup();
    expect(dialog).toContain("set up again");
    await screen.findByText("Cloud backup restored. This PC continues as Laptop.");
    expect(restoreRemoteBackup).toHaveBeenCalledWith("backup-1", true);
    expect(refresh).toHaveBeenCalled();
  });

  it("never takes over a PC with history", async () => {
    vi.mocked(getThisPc).mockResolvedValue(thisPc(false));
    const { refresh, restoreRemoteBackup } = renderPage(cloudBackup("laptop"), restored(null));

    expect(await restoreTheBackup()).not.toContain("set up again");
    await screen.findByText("Cloud backup restored.");
    expect(restoreRemoteBackup).toHaveBeenCalledWith("backup-1", false);
    expect(refresh).toHaveBeenCalled();
  });

  it("never takes over unasked when the backup names no PC", async () => {
    vi.mocked(getThisPc).mockResolvedValue(thisPc(true));
    const unnamed = renderPage(cloudBackup(null), restored(null));
    expect(await restoreTheBackup()).not.toContain("set up again");
    await waitFor(() => expect(unnamed.restoreRemoteBackup).toHaveBeenCalledWith("backup-1", false));
  });

  it("does not take over when this PC cannot be checked", async () => {
    vi.mocked(getThisPc).mockRejectedValue(new Error("the database is locked"));
    const { restoreRemoteBackup } = renderPage(cloudBackup("laptop"), restored(null));
    expect(await restoreTheBackup()).not.toContain("set up again");
    await waitFor(() => expect(restoreRemoteBackup).toHaveBeenCalledWith("backup-1", false));
  });
});
