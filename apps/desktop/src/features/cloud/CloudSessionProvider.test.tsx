// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsContext } from "@/features/settings/settings-context";
import {
  checkCloudServer,
  clearCloudBackupKeySecure,
  clearCloudSessionSecure,
  cloudRequest,
  isDevelopmentBuild,
  loadCloudSessionSecure,
  mergeDueCloudBackups,
  uploadRemoteBackup,
} from "@/lib/tauri";
import type { CloudServerInfo } from "@/lib/types";
import { CloudSessionProvider } from "./CloudSessionProvider";
import { ServerForm } from "./ServerForm";

vi.mock("@tauri-apps/plugin-log", () => ({ info: vi.fn(async () => {}), warn: vi.fn(async () => {}) }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@/lib/tauri", () => ({
  checkCloudServer: vi.fn(),
  clearCloudBackupKeySecure: vi.fn(async () => true),
  clearCloudSessionSecure: vi.fn(async () => true),
  cloudRequest: vi.fn(),
  getAppVersion: vi.fn(async () => "0.5.0"),
  getBackupDeviceId: vi.fn(async () => "this-pc"),
  isDevelopmentBuild: vi.fn(async () => false),
  mergeDueCloudBackups: vi.fn(async () => []),
  hasCloudBackupKeySecure: vi.fn(async () => true),
  loadCloudSessionSecure: vi.fn(),
  restoreRemoteBackup: vi.fn(),
  storeCloudBackupKeySecure: vi.fn(async () => true),
  storeCloudSessionSecure: vi.fn(async () => true),
  uploadRemoteBackup: vi.fn(),
}));

const SESSION = JSON.stringify({
  access_token: "access",
  refresh_token: "refresh",
  expires_at: "2099-01-01T00:00:00.000Z",
  refresh_expires_at: "2099-01-01T00:00:00.000Z",
  user: { id: "account-1", email: "player@example.com" },
  server: "https://vaultime.example.com",
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
  return async (_server: string, _method: string, path: string) => {
    const body =
      path === "/v1/backups"
        ? backups
        : path === "/v1/auth/refresh"
          ? JSON.parse(SESSION)
          : { id: "device", client_device_id: "client", device_name: "PC", platform: "windows", app_version: "0.5.0" };
    return { kind: "answer" as const, status: 200, body };
  };
}

describe("the daily cloud backup", () => {
  beforeEach(() => {
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(SESSION);
    vi.mocked(uploadRemoteBackup).mockReset();
    vi.mocked(mergeDueCloudBackups).mockClear();
  });

  it("uploads when this PC's newest backup is a day old, whatever another PC uploaded", async () => {
    vi.mocked(cloudRequest).mockImplementation(server([backupOf("laptop", 1), backupOf("this-pc", 30)]));
    start({ cloud_backup: "true" });
    await waitFor(() => expect(uploadRemoteBackup).toHaveBeenCalled());
  });

  it("waits while this PC uploaded within the day, and merges what is due", async () => {
    vi.mocked(cloudRequest).mockImplementation(server([backupOf("laptop", 30), backupOf("this-pc", 2)]));
    start({ cloud_backup: "true" });
    await waitFor(() => expect(mergeDueCloudBackups).toHaveBeenCalled());
    expect(uploadRemoteBackup).not.toHaveBeenCalled();
  });
});

describe("the server of the Cloud page", () => {
  const first: CloudServerInfo = { address: "https://first.example.com", signups_open: true };
  const second: CloudServerInfo = { address: "https://second.example.com", signups_open: true };

  /** A signed out PC that connected to `first` before. */
  function startSignedOut() {
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(null);
    const save = vi.fn(async () => {});
    render(
      <SettingsContext.Provider
        value={{ values: { cloud_backup: "true", cloud_server: first.address }, read: true, save, reload: vi.fn(async () => {}), reloads: 0 }}
      >
        <CloudSessionProvider>
          <ServerForm />
        </CloudSessionProvider>
      </SettingsContext.Provider>,
    );
    return save;
  }

  function connectTo(address: string) {
    fireEvent.change(screen.getByLabelText("Server address"), { target: { value: address } });
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
  }

  beforeEach(() => {
    vi.mocked(checkCloudServer).mockReset();
  });

  it("brings back the last server once, so Change stays on the address field", async () => {
    vi.mocked(checkCloudServer).mockResolvedValue(first);
    startSignedOut();
    fireEvent.click(await screen.findByRole("button", { name: "Change" }));
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.getByLabelText("Server address")).toBeTruthy();
    expect(checkCloudServer).toHaveBeenCalledTimes(1);
  });

  it("keeps the newer server when an older check answers late", async () => {
    let answerFirst: (info: CloudServerInfo) => void = () => {};
    vi.mocked(checkCloudServer).mockImplementation((address) =>
      address === first.address ? new Promise((resolve) => (answerFirst = resolve)) : Promise.resolve(second),
    );
    const save = startSignedOut();
    await waitFor(() => expect(checkCloudServer).toHaveBeenCalledWith(first.address));
    connectTo("second.example.com");
    expect(await screen.findByText("second.example.com")).toBeTruthy();
    answerFirst(first);
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(screen.queryByText("first.example.com")).toBeNull();
    expect(save).not.toHaveBeenCalledWith("cloud_server", first.address);
  });
});

describe("deciding cloud backup on a PC from before the switch", () => {
  beforeEach(() => {
    vi.mocked(loadCloudSessionSecure).mockReset();
    // The server stays out of reach, a renewal fails without a word.
    vi.mocked(cloudRequest).mockResolvedValue({ kind: "unreachable" });
  });

  it("keeps it on while this PC is signed in", async () => {
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(SESSION);
    const save = start();
    await waitFor(() => expect(save).toHaveBeenCalledWith("cloud_backup", "true"));
  });

  it("forgets a sign-in to the hosted server, which names no server, with its backup key", async () => {
    const hosted = JSON.parse(SESSION) as Record<string, unknown>;
    delete hosted.server;
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(JSON.stringify(hosted));
    vi.mocked(clearCloudSessionSecure).mockClear();
    vi.mocked(clearCloudBackupKeySecure).mockClear();
    const save = start();
    await waitFor(() => expect(save).toHaveBeenCalledWith("cloud_backup", "false"));
    expect(clearCloudSessionSecure).toHaveBeenCalled();
    expect(clearCloudBackupKeySecure).toHaveBeenCalledWith("account-1");
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

  it("decides nothing in a development build, whose keyring holds a sign-in of its own", async () => {
    vi.mocked(isDevelopmentBuild).mockResolvedValueOnce(true);
    vi.mocked(loadCloudSessionSecure).mockResolvedValue(null);
    const save = start();
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
