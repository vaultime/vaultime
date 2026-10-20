// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { render, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SettingsContext } from "@/features/settings/settings-context";
import { loadCloudSessionSecure } from "@/lib/tauri";
import { CloudSessionProvider } from "./CloudSessionProvider";

vi.mock("@tauri-apps/plugin-log", () => ({ info: vi.fn(async () => {}), warn: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({
  clearCloudBackupKeySecure: vi.fn(async () => true),
  clearCloudSessionSecure: vi.fn(async () => true),
  getAppVersion: vi.fn(async () => "0.5.0"),
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
