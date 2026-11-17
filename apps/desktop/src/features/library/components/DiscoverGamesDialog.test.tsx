// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import * as api from "@/lib/tauri";
import type { SeenGame } from "@/lib/types";
import { DiscoverGamesDialog } from "./DiscoverGamesDialog";

vi.mock("@/lib/tauri", () => ({
  listSeenGames: vi.fn(),
  importDiscoveredGames: vi.fn(async () => []),
  ignoreProgram: vi.fn(async () => {}),
  discoverSteamGames: vi.fn(async () => []),
  discoverLauncherGames: vi.fn(async () => []),
  getDefaultScanPaths: vi.fn(async () => []),
  discoverGames: vi.fn(async () => []),
  rememberOfferedGames: vi.fn(async () => 0),
}));

function seen(title: string): SeenGame {
  return {
    title,
    executable_path: `C:/Games/${title}/${title}.exe`,
    install_folder: `C:/Games/${title}`,
    source: "folder_scan",
    source_id: null,
    already_added: false,
    same_game: null,
    front_seconds: 25 * 60,
  };
}

async function openWith(games: SeenGame[]) {
  vi.mocked(api.listSeenGames).mockResolvedValue(games);
  const onImported = vi.fn();
  render(<DiscoverGamesDialog open onOpenChange={() => {}} onImported={onImported} />);
  return { onImported, list: await screen.findByRole("region", { name: "Seen running" }) };
}

describe("programs seen running in Discover", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("adds one as a game", async () => {
    const tunic = seen("Tunic");
    const { onImported, list } = await openWith([tunic, seen("Hades")]);
    expect(within(list).getAllByText("Ran 25m")).toHaveLength(2);
    fireEvent.click(within(list).getAllByRole("button", { name: "Add" })[0]);
    await waitFor(() => expect(onImported).toHaveBeenCalled());
    expect(api.importDiscoveredGames).toHaveBeenCalledWith([tunic]);
    expect(within(list).queryByText("Tunic")).toBeNull();
    expect(within(list).getByText("Hades")).toBeDefined();
  });

  it("forgets one that is no game", async () => {
    const { list } = await openWith([seen("Overlay")]);
    const changed = vi.fn();
    window.addEventListener("vaultime-seen-changed", changed);
    fireEvent.click(within(list).getByRole("button", { name: "Not a game" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Seen running" })).toBeNull());
    expect(api.ignoreProgram).toHaveBeenCalledWith("C:/Games/Overlay/Overlay.exe", "Overlay");
    expect(changed).toHaveBeenCalled();
    window.removeEventListener("vaultime-seen-changed", changed);
  });

  it("gives way to a launcher's row of the same program after a scan", async () => {
    vi.mocked(api.discoverLauncherGames).mockResolvedValue([{ ...seen("Tunic"), source: "epic", source_id: "tunic" }]);
    await openWith([seen("Tunic")]);
    fireEvent.click(screen.getByRole("button", { name: "Start the scan" }));
    await waitFor(() => expect(screen.queryByRole("region", { name: "Seen running" })).toBeNull());
    expect(screen.getByText("Epic Games")).toBeTruthy();
  });

  it("lists a game once when two readers write its path differently", async () => {
    const steam = { ...seen("Apex"), executable_path: "c:/program files (x86)/steam/steamapps/common/apex/r5apex.exe", source: "steam" };
    const ea = { ...seen("Apex"), executable_path: "C:\\Program Files (x86)\\Steam\\steamapps\\common\\Apex\\r5apex.exe", source: "ea" };
    vi.mocked(api.discoverSteamGames).mockResolvedValue([steam]);
    vi.mocked(api.discoverLauncherGames).mockResolvedValue([ea]);
    vi.mocked(api.listSeenGames).mockResolvedValue([]);
    render(<DiscoverGamesDialog open onOpenChange={() => {}} onImported={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: "Start the scan" }));
    expect(await screen.findByText("1 game found")).toBeTruthy();
    expect(screen.getByText("Steam")).toBeTruthy();
  });

  it("shows nothing of it while none ran", async () => {
    vi.mocked(api.listSeenGames).mockResolvedValue([]);
    render(<DiscoverGamesDialog open onOpenChange={() => {}} onImported={() => {}} />);
    await waitFor(() => expect(api.listSeenGames).toHaveBeenCalled());
    expect(screen.getByRole("button", { name: "Start the scan" })).toBeDefined();
    expect(screen.queryByRole("region", { name: "Seen running" })).toBeNull();
  });
});
