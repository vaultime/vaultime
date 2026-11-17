// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { check } from "@tauri-apps/plugin-updater";
import * as api from "@/lib/tauri";
import { useUpdates } from "./update-context";
import { UpdateProvider } from "./UpdateProvider";

vi.mock("@tauri-apps/plugin-process", () => ({ relaunch: vi.fn() }));
vi.mock("@tauri-apps/plugin-updater", () => ({ check: vi.fn(async () => null) }));
vi.mock("@/lib/tauri", () => ({ updatesManagedBy: vi.fn(async () => null) }));

function Probe() {
  const { status, managedBy } = useUpdates();
  return (
    <p>
      {status} {managedBy}
    </p>
  );
}

describe("UpdateProvider", () => {
  it("looks for updates when Vaultime brings its own", async () => {
    render(
      <UpdateProvider>
        <Probe />
      </UpdateProvider>,
    );
    await waitFor(() => expect(screen.getByText("current")).toBeTruthy());
    expect(check).toHaveBeenCalled();
  });

  it("leaves updates to the package manager that installed Vaultime", async () => {
    vi.mocked(check).mockClear();
    vi.mocked(api.updatesManagedBy).mockResolvedValueOnce("the AUR");
    render(
      <UpdateProvider>
        <Probe />
      </UpdateProvider>,
    );
    await waitFor(() => expect(screen.getByText("managed the AUR")).toBeTruthy());
    expect(check).not.toHaveBeenCalled();
  });
});
