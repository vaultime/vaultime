// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { distinctHues, tintForTitle } from "./game-tint";
import { normalizeIntegrityStatus } from "./integrity";
import { capitalize, numberWords, sentence, whichPc } from "./words";

describe("whichPc", () => {
  it("names this PC and waits for its id", () => {
    expect(whichPc("a", "a")).toBe("this PC");
    expect(whichPc("b", "a")).toBe("another PC");
    expect(whichPc("a", null)).toBeNull();
    expect(whichPc(undefined, "a")).toBeNull();
  });
});

describe("numberWords", () => {
  it("writes numbers up to 999 as words", () => {
    expect(numberWords(0)).toBe("zero");
    expect(numberWords(13)).toBe("thirteen");
    expect(numberWords(40)).toBe("forty");
    expect(numberWords(42)).toBe("forty-two");
    expect(numberWords(100)).toBe("one hundred");
    expect(numberWords(142)).toBe("one hundred forty-two");
    expect(numberWords(999)).toBe("nine hundred ninety-nine");
    expect(numberWords(7.9)).toBe("seven");
  });

  it("uses digits above that", () => {
    expect(numberWords(1000)).toMatch(/^1\D?000$/);
  });

  it("capitalizes the first letter", () => {
    expect(capitalize("twenty hours")).toBe("Twenty hours");
  });

  it("turns a message into a sentence once", () => {
    expect(sentence("invalid email or password")).toBe("Invalid email or password.");
    expect(sentence("Already done.")).toBe("Already done.");
    expect(sentence("really? ")).toBe("Really?");
  });
});

describe("tintForTitle", () => {
  it("gives the same title the same colors", () => {
    expect(tintForTitle("Hades II")).toEqual(tintForTitle("Hades II"));
    expect(tintForTitle("Hades II").fill).not.toBe(tintForTitle("Celeste").fill);
    expect(tintForTitle("Celeste").wash).toMatch(/^oklch\(/);
  });
});

describe("distinctHues", () => {
  const distance = (a: number, b: number) => Math.min(Math.abs(a - b), 360 - Math.abs(a - b));

  it("keeps hues that are already far apart", () => {
    expect(distinctHues([10, 120, 240])).toEqual([10, 120, 240]);
    expect(distinctHues([200])).toEqual([200]);
  });

  it("moves a close hue only as far as it needs to", () => {
    expect(distinctHues([100, 110])).toEqual([100, 150]);
    expect(distinctHues([20, 350])).toEqual([20, 330]);
  });

  it("keeps taken hues free", () => {
    expect(distinctHues([290, 100], [293])).toEqual([243, 100]);
  });

  it("spreads many games around the wheel", () => {
    const hues = distinctHues(Array.from({ length: 10 }, () => 0));
    for (const [index, hue] of hues.entries()) {
      for (const other of hues.slice(index + 1)) expect(distance(hue, other)).toBeGreaterThanOrEqual(36);
    }
  });
});

describe("normalizeIntegrityStatus", () => {
  it("accepts the known labels only", () => {
    expect(normalizeIntegrityStatus(" Suspicious ")).toBe("suspicious");
    expect(normalizeIntegrityStatus("recovered")).toBe("recovered");
    expect(normalizeIntegrityStatus("verified")).toBe("local");
    expect(normalizeIntegrityStatus(null)).toBe("local");
  });
});
