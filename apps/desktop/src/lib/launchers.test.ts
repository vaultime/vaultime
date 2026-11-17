// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS } from "@/lib/constants";
import {
  BY_HAND,
  launcherCounts,
  launchersOfGame,
  launcherTitle,
  launcherWhere,
  timeOnLauncher,
} from "@/lib/launchers";
import type { EarlierPlaytime, Game, GameLink } from "@/lib/types";

const game = (id: string, launcher_source: string | null) => ({ id, launcher_source }) as Game;
const link = (game_id: string, linked_game_id: string, launcher_source: string | null, origin_device_id: string | null = null) =>
  ({ game_id, linked_game_id, launcher_source, origin_device_id }) as GameLink;

describe("launchers", () => {
  it("titles launchers, the folder scan and games added by hand", () => {
    expect(launcherTitle("epic")).toBe("Epic Games");
    expect(launcherTitle("folder_scan")).toBe("Found in folders");
    expect(launcherTitle(BY_HAND)).toBe("Added by hand");
    expect(launcherTitle("someday")).toBe("someday");
    expect(launcherWhere("gog")).toBe("through GOG");
    expect(launcherWhere("folder_scan")).toBe("in games found in folders");
    expect(launcherWhere(BY_HAND)).toBe("in games added by hand");
  });

  it("counts the time a launcher counted before Vaultime on that launcher only", () => {
    const played = {
      launcherMs: { steam: 10 * HOUR_MS, gog: 4 * HOUR_MS },
      earlier: { source: "steam", earlier_ms: 30 * HOUR_MS } as EarlierPlaytime,
    };
    expect(timeOnLauncher(played, "steam")).toBe(40 * HOUR_MS);
    expect(timeOnLauncher(played, "gog")).toBe(4 * HOUR_MS);
    expect(timeOnLauncher(played, "epic")).toBe(0);
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
