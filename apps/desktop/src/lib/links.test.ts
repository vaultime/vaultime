// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { at, session } from "@/test/sessions";
import type { Game, GameLink } from "@/lib/types";
import {
  launcherName,
  launchersOf,
  linkTargets,
  linkedGoToo,
  linkedTo,
  recordedThrough,
  whereFrom,
} from "./links";

function game(id: string, launcher: string | null, origin: string | null = null): Game {
  return {
    id,
    title: "Cyberpunk 2077",
    executable_path: null,
    install_folder: null,
    launcher_source: launcher,
    metadata_json: "{}",
    is_hidden: false,
    created_at: "",
    updated_at: "",
    origin_device_id: origin,
  };
}

function link(gameId: string, launcher: string | null, origin: string | null = null): GameLink {
  return {
    game_id: gameId,
    title: "Cyberpunk 2077",
    origin_device_id: origin,
    linked_game_id: "steam",
    launcher_source: launcher,
    executable_path: `C:/${gameId}/Cyberpunk2077.exe`,
    install_folder: null,
  };
}

const links = [link("gog", "gog"), link("xbox", "xbox", "desktop"), link("laptop", "steam", "laptop")];

describe("links", () => {
  it("names launchers and where a game came from", () => {
    expect(launcherName("gog")).toBe("GOG");
    expect(launcherName("folder_scan")).toBeNull();
    expect(whereFrom("steam")).toBe("on Steam");
    expect(whereFrom("folder_scan")).toBe("found in a folder");
    expect(whereFrom(null)).toBe("added by hand");
  });

  it("tells installs here from games of other PCs", () => {
    const { installs, elsewhere } = linkedTo("steam", links, "desktop");
    expect(installs.map((entry) => entry.game_id)).toEqual(["gog", "xbox"]);
    expect(elsewhere.map((entry) => entry.game_id)).toEqual(["laptop"]);
    expect(linkedTo("other", links, "desktop").installs).toEqual([]);
  });

  it("lists the launchers of every install here once", () => {
    expect(launchersOf(game("steam", "steam"), links, "desktop")).toBe("Steam, GOG and Xbox");
    expect(launchersOf(game("steam", "steam"), [], "desktop")).toBe("Steam");
    expect(launchersOf(game("hand", null), [], "desktop")).toBeNull();
  });

  it("offers every other game of this PC as a target", () => {
    const games = [game("steam", "steam"), game("epic", "epic"), game("laptop-only", "steam", "laptop")];
    expect(linkTargets(games[0], games).map((entry) => entry.id)).toEqual(["epic"]);
  });

  it("says what a delete takes along", () => {
    expect(linkedGoToo(0)).toBe("");
    expect(linkedGoToo(1)).toBe("The game linked to it goes too, with its sessions. Unlink it first to keep it.");
    expect(linkedGoToo(2)).toContain("The 2 games linked to it go too");
  });

  it("names the install here that recorded a session", () => {
    const start = at(2026, 10, 2, 18);
    expect(recordedThrough(session(start, 60, { recorded_game_id: "gog" }), links, "desktop")).toBe("GOG");
    expect(recordedThrough(session(start, 60, { recorded_game_id: "laptop" }), links, "desktop")).toBeNull();
    expect(recordedThrough(session(start, 60), links, "desktop")).toBeNull();
  });
});
