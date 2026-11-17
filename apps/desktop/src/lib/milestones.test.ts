// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS } from "@/lib/constants";
import { at, session } from "@/test/sessions";
import { markAfter, markBefore, marksBetween, marksPassedBy, markWords, nextMark, reachedMarks } from "./milestones";

describe("hour marks", () => {
  it("go through the listed marks, then one every thousand hours", () => {
    expect(markAfter(0)).toBe(10);
    expect(markAfter(10)).toBe(25);
    expect(markAfter(999)).toBe(1000);
    expect(markAfter(1000)).toBe(2000);
    expect(markAfter(2500)).toBe(3000);
    expect(markBefore(10)).toBe(0);
    expect(markBefore(100)).toBe(50);
    expect(markBefore(2000)).toBe(1000);
    expect(markBefore(3000)).toBe(2000);
  });

  it("finds the marks a stretch of play passed", () => {
    expect(marksBetween(9 * HOUR_MS, 26 * HOUR_MS)).toEqual([10, 25]);
    expect(marksBetween(10 * HOUR_MS, 11 * HOUR_MS)).toEqual([]);
    expect(marksBetween(0, 10 * HOUR_MS)).toEqual([10]);
  });

  it("tells how far the next mark is", () => {
    expect(nextMark(75 * HOUR_MS)).toEqual({ mark: 100, progress: 0.5, leftMs: 25 * HOUR_MS });
    expect(nextMark(0)).toEqual({ mark: 10, progress: 0, leftMs: 10 * HOUR_MS });
  });

  it("names marks in words", () => {
    expect(markWords(10)).toBe("ten hours");
    expect(markWords(100)).toBe("a hundred hours");
    expect(markWords(250)).toBe("two hundred fifty hours");
    expect(markWords(1000)).toBe("a thousand hours");
    expect(markWords(3000)).toBe("three thousand hours");
  });
});

describe("reachedMarks", () => {
  it("gives each mark the session that passed it, and none to play before Vaultime", () => {
    const first = session(at(2026, 3, 1, 20, 0), 5 * 60, { id: "first" });
    const second = session(at(2026, 3, 2, 20, 0), 20 * 60, { id: "second" });
    const reached = reachedMarks([second, first], 12 * HOUR_MS);
    expect(reached.map(({ mark, session }) => [mark, session?.id ?? null])).toEqual([
      [10, null],
      [25, "second"],
    ]);
  });
});

describe("marksPassedBy", () => {
  const before = session(at(2026, 3, 1, 20, 0), 9 * 60, { id: "before" });
  const past = session(at(2026, 3, 2, 20, 0), 2 * 60, { id: "past" });

  it("counts the sessions that started before it", () => {
    expect(marksPassedBy(past, [past, before])).toEqual([10]);
    expect(marksPassedBy(before, [past, before])).toEqual([]);
    expect(marksPassedBy(past, undefined)).toEqual([]);
  });

  it("leaves out sessions that count as no play", () => {
    const nothing = session(at(2026, 3, 1, 10, 0), 0, { id: "nothing", runtime_ms: 0, counted_runtime_ms: 0 });
    expect(marksPassedBy(past, [before, nothing, past], 30 * MINUTE_MS)).toEqual([10]);
  });
});
