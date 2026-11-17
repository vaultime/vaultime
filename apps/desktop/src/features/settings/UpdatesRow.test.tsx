// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { UpdateContext, type UpdateState } from "@/features/updates/update-context";
import { UpdatesRow } from "./UpdatesRow";

function renderRow(state: Partial<UpdateState>) {
  const value: UpdateState = {
    status: "current",
    version: null,
    error: null,
    checkedAt: null,
    managedBy: null,
    check: vi.fn(async () => {}),
    install: vi.fn(async () => {}),
    ...state,
  };
  render(
    <UpdateContext.Provider value={value}>
      <UpdatesRow />
    </UpdateContext.Provider>,
  );
}

describe("UpdatesRow", () => {
  it("offers to look for a new version", () => {
    renderRow({});
    expect(screen.getByRole("button", { name: "Check now" })).toBeTruthy();
  });

  it("names the package manager that brings the updates and offers nothing to click", () => {
    renderRow({ status: "managed", managedBy: "the AUR" });
    expect(screen.getByText("Updates come through the AUR.")).toBeTruthy();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
