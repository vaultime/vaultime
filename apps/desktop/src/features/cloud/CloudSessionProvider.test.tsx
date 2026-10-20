// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsContext } from "@/features/settings/settings-context";
import { loadCloudSessionSecure, mergeDueCloudBackups, uploadRemoteBackup } from "@/lib/tauri";
import { CloudSessionProvider } from "./CloudSessionProvider";

vi.mock("@tauri-apps/plugin-log", () => ({ info: vi.fn(async () => {}), warn: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({
  clearCloudBackupKeySecure: vi.fn(async () => true),
  clearCloudSessionSecure: vi.fn(async () => true),
  getAppVersion: vi.fn(async () => "0.5.0"),
  getDeviceId: vi.fn(async () => "this-pc"),
  mergeDueCloudBackups: vi.fn(async () => []),
  hasCloudBackupKeySecure: vi.fn(async () => true),
  loadCloudSessionSecure: vi.fn(),
  restoreRemoteBackup: vi.fn(),
  setCloudSignedIn: vi.fn(async () => {}),
  storeCloudBackupKeySecure: vi.fn(async () => true),
  storeCloudSessionSecure: vi.fn(async () => true),
  uploadRemoteBackup: vi.fn(),
}));

const SESSION = JSON.stringify({
  access_token: "access",
  refresh_token: "refresh",
  expires_at: "2099-01-01T00:00:00.000Z",
  refresh_expires_at: "2099-01-01T00:00:00.000Z",
  user: { id: "account-1", email: "player@example.com", role: "user" },
});

/** The provider on a PC from before the cloud switch, whose settings hold none. */
function start(values: Record<string, string> = {}, read = true) {
  const save = vi.fn(async () => {});
  render(
    <SettingsContext.Provider value={{ values, read, save, reload: vi.fn(async () => {}), reloads: 0 }}>
      <CloudSessionProvider>
        <span />
      </CloudSessionProvider>
    </SettingsContext.Provider>,
  );
  return save;
}

const HOUR_MS = 60 * 60 * 1000;

/** A complete backup of `pc`, uploaded `hoursAgo`. */
function backupOf(pc: string, hoursAgo: number) {
  return {
    id: `${pc}-${hoursAgo}`,
    label: null,
    storage_key: "key",
    checksum: "sum",
    size_bytes: 1,
    backup_created_at: new Date(Date.now() - hoursAgo * HOUR_MS).toISOString(),
    uploaded_at: new Date(Date.now() - hoursAgo * HOUR_MS).toISOString(),
    status: "complete",
    client_device_id: null,
    metadata_json: { source_device_id: pc },
  };
}

/** A server that lists `backups`, renews the session and registers any PC. */
function server(backups: unknown[]) {
  return vi.fn(async (url: URL | string) => {
    const path = new URL(url).pathname;
    const body =
      path === "/v1/backups"
        ? backups
        : path === "/v1/auth/refresh"
          ? JSON.parse(SESSION)
          : { id: "device", client_device_id: "client", device_name: "PC", platform: "windows", app_version: "0.5.0" };
    return new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } });
  });
}

describe("the daily cloud backup", () => {
  beforeEach(() => {
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(SESSION);
    vi.mocked(uploadRemoteBackup).mockReset();
    vi.mocked(mergeDueCloudBackups).mockClear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("uploads when this PC's newest backup is a day old, whatever another PC uploaded", async () => {
    vi.stubGlobal("fetch", server([backupOf("laptop", 1), backupOf("this-pc", 30)]));
    start({ cloud_backup: "true" });
    await waitFor(() => expect(uploadRemoteBackup).toHaveBeenCalled());
  });

  it("waits while this PC uploaded within the day, and merges what is due", async () => {
    vi.stubGlobal("fetch", server([backupOf("laptop", 30), backupOf("this-pc", 2)]));
    start({ cloud_backup: "true" });
    await waitFor(() => expect(mergeDueCloudBackups).toHaveBeenCalled());
    expect(uploadRemoteBackup).not.toHaveBeenCalled();
  });
});

describe("deciding cloud backup on a PC from before the switch", () => {
  beforeEach(() => {
    vi.mocked(loadCloudSessionSecure).mockReset();
    // The server stays out of reach, a renewal fails without a word.
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("Failed to fetch")));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("keeps it on while this PC is signed in", async () => {
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(SESSION);
    const save = start();
    await waitFor(() => expect(save).toHaveBeenCalledWith("cloud_backup", "true"));
  });

  it("turns it off when the keyring holds no sign-in", async () => {
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(null);
    const save = start();
    await waitFor(() => expect(save).toHaveBeenCalledWith("cloud_backup", "false"));
  });

  it("decides nothing while the keyring cannot be read", async () => {
    vi.mocked(loadCloudSessionSecure).mockRejectedValue(new Error("the keyring is locked"));
    const save = start();
    await waitFor(() => expect(loadCloudSessionSecure).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(save).not.toHaveBeenCalled();
  });

  it("decides nothing when the settings could not be read", async () => {
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(null);
    const save = start({}, false);
    await waitFor(() => expect(loadCloudSessionSecure).toHaveBeenCalled());
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(save).not.toHaveBeenCalled();
  });

  it("never reads the keyring while it is off", async () => {
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(SESSION);
    const save = start({ cloud_backup: "false" });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(loadCloudSessionSecure).not.toHaveBeenCalled();
    expect(save).not.toHaveBeenCalled();
  });
});
