// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CloudDevice } from "@/lib/types";
import { CloudSessionContext, type CloudSessionContextValue } from "./cloud-context";
import { RegisteredPcsSection } from "./RegisteredPcsSection";

function pc(id: string, name: string): CloudDevice {
  return {
    id,
    client_device_id: `client-${id}`,
    device_name: name,
    platform: "windows",
    app_version: "0.6.0",
    registered_at: "2026-10-01T10:00:00.000Z",
    last_seen_at: "2026-10-19T10:00:00.000Z",
  };
}

describe("the registered PCs", () => {
  it("lists every PC and removes the others, never this one", async () => {
    const removeDevice = vi.fn(async () => {});
    const cloud = {
      device: pc("this", "Desktop"),
      listDevices: vi.fn(async () => [pc("this", "Desktop"), pc("old", "Old laptop")]),
      removeDevice,
    } as unknown as CloudSessionContextValue;
    render(
      <CloudSessionContext.Provider value={cloud}>
        <RegisteredPcsSection onError={vi.fn()} />
      </CloudSessionContext.Provider>,
    );

    expect(await screen.findByText("Desktop, this PC")).toBeDefined();
    const remove = screen.getAllByRole("button", { name: "Remove" });
    expect(remove).toHaveLength(1);
    fireEvent.click(remove[0]);
    await waitFor(() => expect(removeDevice).toHaveBeenCalledWith("old"));
    await waitFor(() => expect(screen.queryByText("Old laptop")).toBeNull());
  });
});
