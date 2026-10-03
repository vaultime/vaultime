// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { at } from "@/test/sessions";
import type { MergePreview } from "@/lib/types";
import { mergeChangesSomething, mergeDone, mergeHeadline, mergeNotes } from "./merge";

const preview: MergePreview = {
  backup_path: "C:/backups/laptop",
  backup_created_at: at(2026, 10, 3, 12),
  device_id: "laptop-id",
  device_name: "Laptop",
  new_sessions: 312,
  grown_sessions: 0,
  already_here: 0,
  newer_here: 0,
  removed_here: 0,
  running_there: 0,
  failing: 0,
  unknown_keys: false,
  first_merge: false,
  vouched_now: 0,
  overlapping_manual: 0,
  conflicts: [],
  games: [],
};

describe("merge text", () => {
  it("says what comes in", () => {
    expect(mergeHeadline(preview)).toMatch(/^From Laptop's backup of .+ come 312 sessions that are not here yet\.$/);
    expect(mergeHeadline({ ...preview, grown_sessions: 1 })).toContain(
      "312 sessions that are not here yet and 1 session that went on there",
    );
    expect(mergeHeadline({ ...preview, new_sessions: 0 })).toMatch(/^Nothing new to merge from Laptop's backup/);
  });

  it("counts sessions here that pass their check now", () => {
    expect(mergeHeadline({ ...preview, new_sessions: 0, vouched_now: 2 })).toMatch(
      /^With Laptop's backup of .+, 2 sessions here pass their check now\.$/,
    );
    expect(mergeHeadline({ ...preview, vouched_now: 1 })).toMatch(
      /come 312 sessions that are not here yet\. 1 session here passes its check now\.$/,
    );
    expect(mergeChangesSomething({ ...preview, new_sessions: 0 })).toBe(false);
    expect(mergeChangesSomething({ ...preview, new_sessions: 0, vouched_now: 1 })).toBe(true);
  });

  it("names what stays as it is and what comes in unvouched", () => {
    expect(mergeNotes(preview)).toEqual([]);
    const notes = mergeNotes({
      ...preview,
      failing: 1,
      unknown_keys: true,
      first_merge: true,
      overlapping_manual: 2,
      running_there: 2,
      removed_here: 1,
      already_here: 3,
      conflicts: [
        { session_id: "a", game_title: "Hades", started_at_wall: at(2026, 10, 1, 18), reason: "changed_on_both" },
      ],
    });
    expect(notes).toEqual([
      "Vaultime has not seen Laptop before, so it takes its ledger at its word, as it does for any new PC.",
      "1 session does not pass its check here, so it comes in Suspicious.",
      "Laptop signs with a key this PC does not trust for it, so that key vouches for nothing here unless you say so below.",
      "2 sessions you added by hand here overlap sessions that come in. If they were the same play, take them out after the merge.",
      "1 session went on differently on both PCs and stays as here.",
      "2 sessions still ran there and come with the next merge.",
      "1 session you removed here stays away.",
      "3 sessions are here already.",
    ]);
  });

  it("says what a merge did", () => {
    const summary = {
      device_id: "laptop-id",
      device_name: "Laptop",
      sessions_added: 2,
      sessions_grown: 1,
      sessions_vouched: 0,
      failing: 0,
      games_added: 1,
      games_linked: 1,
      safety_backup_path: null,
    };
    expect(mergeDone(summary)).toBe("Merged 3 sessions from Laptop.");
    expect(mergeDone({ ...summary, sessions_added: 0, sessions_grown: 0 })).toBe("Nothing new came from Laptop.");
    expect(mergeDone({ ...summary, sessions_vouched: 2 })).toBe(
      "Merged 3 sessions from Laptop. This PC vouches for 2 more sessions of Laptop now.",
    );
    expect(mergeDone({ ...summary, sessions_added: 0, sessions_grown: 0, sessions_vouched: 1 })).toBe(
      "This PC vouches for 1 more session of Laptop now.",
    );
  });
});
