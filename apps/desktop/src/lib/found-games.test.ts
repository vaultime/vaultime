// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import type { DiscoveredGame } from "@/lib/types";
import { foundGamesText, offeredPaths, otherDialogOpen } from "./found-games";

function found(title: string, alreadyAdded = false): DiscoveredGame {
  return {
    title,
    executable_path: `C:/Games/${title}/${title}.exe`,
    install_folder: `C:/Games/${title}`,
    source: "steam",
    source_id: null,
    already_added: alreadyAdded,
  };
}

describe("offeredPaths", () => {
  it("leaves out games already in the library", () => {
    expect(offeredPaths([found("Celeste"), found("Balatro", true)])).toEqual(["C:/Games/Celeste/Celeste.exe"]);
  });
});

describe("otherDialogOpen", () => {
  it("waits for the add and discover dialogs and the palette", () => {
    expect(otherDialogOpen(new URLSearchParams("discover=1"), false)).toBe(true);
    expect(otherDialogOpen(new URLSearchParams("add=1"), false)).toBe(true);
    expect(otherDialogOpen(new URLSearchParams(""), true)).toBe(true);
    expect(otherDialogOpen(new URLSearchParams("tab=all"), false)).toBe(false);
  });
});

describe("foundGamesText", () => {
  it("says the games are new since last time once discovery showed some before", () => {
    const text = foundGamesText({ games: [found("Celeste"), found("Balatro")], since_last: true });
    expect(text.title).toBe("New games found");
    expect(text.description).toContain("found these since last time");
  });

  it("speaks of the library the first time and of one game alone", () => {
    const text = foundGamesText({ games: [found("Celeste")], since_last: false });
    expect(text.title).toBe("A new game found");
    expect(text.description).toContain("This game is installed but not in your library yet.");
  });
});
