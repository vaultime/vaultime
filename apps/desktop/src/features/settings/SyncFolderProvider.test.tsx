// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { useSyncFolder } from "@/features/settings/sync-folder-context";
import * as api from "@/lib/tauri";
import type { AutoMerge } from "@/lib/types";
import { SyncFolderProvider } from "./SyncFolderProvider";

vi.mock("@tauri-apps/plugin-log", () => ({ warn: vi.fn(async () => {}) }));
vi.mock("@/lib/tauri", () => ({ mergeDueSyncBackups: vi.fn() }));

function merge(deviceId: string, outcome: AutoMerge["outcome"]): AutoMerge {
  return { device_id: deviceId, name: null, backup_id: `${deviceId}-backup`, outcome, sessions_added: 0, sessions_grown: 0 };
}

function Waiting() {
  const { waiting } = useSyncFolder();
  return <p>Waiting: {waiting.map((each) => each.device_id).join(", ") || "none"}</p>;
}

describe("the hourly look into the sync folder", () => {
  it("merges what is due at start and keeps the backups that wait for the player", async () => {
    vi.mocked(api.mergeDueSyncBackups).mockResolvedValue([merge("laptop", "merged"), merge("deck", "new_key")]);
    render(
      <SyncFolderProvider>
        <Waiting />
      </SyncFolderProvider>,
    );

    expect(await screen.findByText("Waiting: deck")).toBeTruthy();
    expect(api.mergeDueSyncBackups).toHaveBeenCalledTimes(1);
  });
});
