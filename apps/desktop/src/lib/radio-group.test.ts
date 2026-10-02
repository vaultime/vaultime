// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { nextRadio, radioTabIndex } from "./radio-group";

describe("nextRadio", () => {
  it("moves with the arrow keys and wraps around", () => {
    expect(nextRadio("ArrowRight", 0, 3)).toBe(1);
    expect(nextRadio("ArrowDown", 2, 3)).toBe(0);
    expect(nextRadio("ArrowLeft", 0, 3)).toBe(2);
    expect(nextRadio("ArrowUp", 1, 3)).toBe(0);
  });

  it("jumps to the ends with Home and End", () => {
    expect(nextRadio("Home", 2, 3)).toBe(0);
    expect(nextRadio("End", 0, 3)).toBe(2);
  });

  it("leaves other keys alone", () => {
    expect(nextRadio("Enter", 0, 3)).toBeNull();
    expect(nextRadio("ArrowRight", 0, 0)).toBeNull();
  });
});

describe("radioTabIndex", () => {
  it("makes the checked radio the one tab stop", () => {
    expect([false, true, false].map((checked, index) => radioTabIndex(checked, index, true))).toEqual([-1, 0, -1]);
  });

  it("makes the first radio the tab stop when none is checked", () => {
    expect([false, false].map((checked, index) => radioTabIndex(checked, index, false))).toEqual([0, -1]);
  });
});
