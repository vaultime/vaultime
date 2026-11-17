// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { BY_HAND, launcherCounts, launchersOfGame, launcherTitle } from "@/lib/launchers";
import type { Game, GameLink } from "@/lib/types";

const game = (id: string, launcher_source: string | null) => ({ id, launcher_source }) as Game;
const link = (game_id: string, linked_game_id: string, launcher_source: string | null, origin_device_id: string | null = null) =>
  ({ game_id, linked_game_id, launcher_source, origin_device_id }) as GameLink;

describe("launchers", () => {
  it("titles launchers, the folder scan and games added by hand", () => {
    expect(launcherTitle("epic")).toBe("Epic Games");
    expect(launcherTitle("folder_scan")).toBe("Found in folders");
    expect(launcherTitle(BY_HAND)).toBe("Added by hand");
    expect(launcherTitle("someday")).toBe("someday");
  });

  it("counts a game for the launcher of every copy, here or on another PC", () => {
    const links = [
      link("cyberpunk-gog", "cyberpunk", "gog"),
      link("cyberpunk-laptop", "cyberpunk", "steam", "laptop"),
      link("tetris-laptop", "tetris", "folder_scan", "laptop"),
    ];
    expect(launchersOfGame(game("cyberpunk", "steam"), links)).toEqual(["steam", "gog"]);
    expect(launchersOfGame(game("tetris", null), links)).toEqual([BY_HAND, "folder_scan"]);
  });

  it("lists launchers with the most games first and what is no launcher last", () => {
    const counts = launcherCounts([
      { launchers: ["steam", "gog"] },
      { launchers: ["steam"] },
      { launchers: ["folder_scan"] },
      { launchers: ["folder_scan"] },
      { launchers: ["folder_scan"] },
      { launchers: [BY_HAND] },
      { launchers: ["epic"] },
    ]);
    expect(counts).toEqual([
      { launcher: "steam", count: 2 },
      { launcher: "epic", count: 1 },
      { launcher: "gog", count: 1 },
      { launcher: "folder_scan", count: 3 },
      { launcher: BY_HAND, count: 1 },
    ]);
  });
});
