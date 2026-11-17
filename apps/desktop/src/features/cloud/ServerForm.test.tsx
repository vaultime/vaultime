// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { CloudSessionContext, type CloudSessionContextValue } from "@/features/cloud/cloud-context";
import { SettingsContext } from "@/features/settings/settings-context";
import type { CloudServerInfo } from "@/lib/types";
import { ServerForm } from "./ServerForm";

vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: vi.fn() }));
vi.mock("@tauri-apps/plugin-log", () => ({ warn: vi.fn(async () => {}) }));

const OPEN: CloudServerInfo = { address: "https://vaultime.example.com", signups_open: true };

/** The form with a provider that keeps the server it connects to. */
function renderForm(check: (address: string) => Promise<CloudServerInfo>, saved?: string) {
  const connect = vi.fn(check);
  const reconnect = vi.fn();
  function Cloud() {
    const [server, setServer] = useState<CloudServerInfo | null>(null);
    const cloud = {
      server,
      connectServer: async (address: string) => {
        const info = await connect(address);
        setServer(info);
        return info;
      },
      forgetServer: () => setServer(null),
      reconnectServer: reconnect,
    } as unknown as CloudSessionContextValue;
    return (
      <CloudSessionContext.Provider value={cloud}>
        <ServerForm />
      </CloudSessionContext.Provider>
    );
  }
  const values: Record<string, string> = saved ? { cloud_server: saved } : {};
  render(
    <SettingsContext.Provider value={{ values, read: true, save: vi.fn(async () => {}), reload: vi.fn(async () => {}), reloads: 0 }}>
      <Cloud />
    </SettingsContext.Provider>,
  );
  return { connect, reconnect };
}

function connectTo(address: string) {
  fireEvent.change(screen.getByLabelText("Server address"), { target: { value: address } });
  fireEvent.click(screen.getByRole("button", { name: "Connect" }));
}

describe("ServerForm", () => {
  it("connects to the server typed in and says whether it takes new accounts", async () => {
    const { connect } = renderForm(async () => OPEN);
    connectTo("vaultime.example.com");
    expect(await screen.findByText("vaultime.example.com")).toBeTruthy();
    expect(screen.getByText("It takes new accounts.")).toBeTruthy();
    expect(connect).toHaveBeenCalledWith("vaultime.example.com");

    fireEvent.click(screen.getByRole("button", { name: "Change" }));
    expect(screen.getByLabelText("Server address")).toBeTruthy();
  });

  it("says why it did not take a server", async () => {
    renderForm(async () => {
      throw new Error("no Vaultime server answers at that address.");
    });
    connectTo("example.com");
    expect(await screen.findByText("No Vaultime server answers at that address.")).toBeTruthy();
  });

  it("asks for an address before it connects", async () => {
    const { connect } = renderForm(async () => OPEN);
    connectTo(" ");
    expect(await screen.findByText("Enter the address of the server.")).toBeTruthy();
    expect(connect).not.toHaveBeenCalled();
  });

  it("starts from the server of the last sign-in", async () => {
    const { reconnect } = renderForm(async () => OPEN, OPEN.address);
    expect((screen.getByLabelText("Server address") as HTMLInputElement).value).toBe(OPEN.address);
    await waitFor(() => expect(reconnect).toHaveBeenCalledTimes(1));
  });
});
