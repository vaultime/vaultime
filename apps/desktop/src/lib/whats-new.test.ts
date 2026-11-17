// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { version } from "../../src-tauri/tauri.conf.json";
import { FIXES_HEADING, loadReleaseNotes, releaseNotes } from "./whats-new";

const CHANGELOG = [
  "# Changelog",
  "",
  "## [Unreleased]",
  "",
  "### Library",
  "",
  "- **Seen running.** A game from your [game folders](https://example.org) shows in *Discover*.",
  "",
  "## [0.6.0] - 2026-10-20",
  "",
  "### Interface",
  "",
  "- **Year in games:** your year in `chapters`,",
  "  one card at a time.",
  "- Shorter times everywhere.",
  "",
  "### Fixes",
  "",
  "- Sessions after a crash.",
  "",
  "## [0.5.0] - 2026-09-01",
  "",
].join("\r\n");

describe("the release notes", () => {
  it("come from the version's section, in plain words", () => {
    expect(releaseNotes(CHANGELOG, "0.6.0")).toEqual({
      version: "0.6.0",
      date: "2026-10-20",
      groups: [
        {
          heading: "Interface",
          notes: [
            { title: "Year in games", text: "your year in chapters, one card at a time." },
            { title: "", text: "Shorter times everywhere." },
          ],
        },
        { heading: "Fixes", notes: [{ title: "", text: "Sessions after a crash." }] },
      ],
    });
  });

  it("come from the unreleased section before the release commit", () => {
    expect(releaseNotes(CHANGELOG, "0.7.0")).toEqual({
      version: "0.7.0",
      date: null,
      groups: [{ heading: "Library", notes: [{ title: "Seen running", text: "A game from your game folders shows in Discover." }] }],
    });
  });

  it("of this version are in the changelog the build carries", async () => {
    const release = await loadReleaseNotes(version);
    expect(release?.groups.some((group) => group.heading !== FIXES_HEADING && group.notes.length > 0)).toBe(true);
  });

  it("are missing for a section without notes", () => {
    expect(releaseNotes(CHANGELOG, "0.5.0")).toBeNull();
    expect(releaseNotes("# Changelog", "0.5.0")).toBeNull();
  });
});
