// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
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

describe("normalizeIntegrityStatus", () => {
  it("accepts the known labels only", () => {
    expect(normalizeIntegrityStatus(" Suspicious ")).toBe("suspicious");
    expect(normalizeIntegrityStatus("recovered")).toBe("recovered");
    expect(normalizeIntegrityStatus("verified")).toBe("local");
    expect(normalizeIntegrityStatus(null)).toBe("local");
  });
});
