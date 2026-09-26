// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { describe, expect, it } from "vitest";
import { tintForTitle } from "./game-tint";
import { normalizeIntegrityStatus } from "./integrity";
import { capitalize, numberWords } from "./words";

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
});

describe("tintForTitle", () => {
  it("gives the same title the same colors", () => {
    expect(tintForTitle("Hades II")).toEqual(tintForTitle("Hades II"));
    expect(tintForTitle("Hades II").fill).not.toBe(tintForTitle("Celeste").fill);
    expect(tintForTitle("Celeste").wash).toMatch(/^oklch\(/);
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
