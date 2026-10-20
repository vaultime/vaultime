// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ContextType } from "react";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { LibraryContext } from "@/features/library/library-context";
import type { MergePreview, MergeSummary } from "@/lib/types";
import { MergeDialog } from "./MergeDialog";

const PREVIEW: MergePreview = {
  backup_path: "",
  backup_created_at: "2026-10-20T10:00:00.000Z",
  device_id: "laptop",
  device_name: "Laptop",
  new_sessions: 2,
  grown_sessions: 0,
  already_here: 0,
  newer_here: 0,
  removed_here: 0,
  running_there: 0,
  failing: 0,
  unknown_keys: false,
  first_merge: false,
  vouched_now: 0,
  own_failing: 0,
  overlapping_manual: 0,
  conflicts: [],
  games: [],
};

const SUMMARY: MergeSummary = {
  device_id: "laptop",
  device_name: "Laptop",
  sessions_added: 2,
  sessions_grown: 0,
  sessions_vouched: 0,
  failing: 0,
  games_added: 0,
  games_linked: 0,
  safety_backup_path: null,
};

describe("the merge dialog", () => {
  it("stays open while a merge runs", async () => {
    let finish: (summary: MergeSummary) => void = () => {};
    const mergeWith = vi.fn(() => new Promise<MergeSummary>((resolve) => (finish = resolve)));
    const onClose = vi.fn();
    const onMerged = vi.fn();
    const library = { games: [] } as unknown as NonNullable<ContextType<typeof LibraryContext>>;
    render(
      <LibraryContext.Provider value={library}>
        <MergeDialog
          preview={PREVIEW}
          readAgain={vi.fn(async () => PREVIEW)}
          mergeWith={mergeWith}
          onClose={onClose}
          onMerged={onMerged}
        />
      </LibraryContext.Provider>,
    );

    fireEvent.click(screen.getByRole("button", { name: "Merge" }));
    const cancel = screen.getByRole("button", { name: "Cancel" });
    await waitFor(() => expect(cancel.hasAttribute("disabled")).toBe(true));
    fireEvent.click(cancel);
    fireEvent.keyDown(document.activeElement ?? document.body, { key: "Escape" });
    expect(onClose).not.toHaveBeenCalled();

    finish(SUMMARY);
    await waitFor(() => expect(onMerged).toHaveBeenCalled());
    await waitFor(() => expect(cancel.hasAttribute("disabled")).toBe(false));
  });
});
