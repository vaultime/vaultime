// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS } from "@/lib/constants";
import type { LedgerReport } from "@/lib/types";
import { phraseString } from "@/test/phrases";
import {
  gamePlaytime,
  lastPlayedLine,
  ledgerSentence,
  libraryPlaytime,
  sideBySideSentence,
  statusSentence,
  weekSentence,
} from "./sentences";

describe("libraryPlaytime", () => {
  it("puts the week in italics", () => {
    expect(libraryPlaytime(142 * HOUR_MS, 11 * HOUR_MS)).toEqual({
      before: "One hundred forty-two hours so far, ",
      em: "eleven",
      after: " of them this past week.",
    });
  });

  it("handles short and brand new totals", () => {
    expect(libraryPlaytime(5 * HOUR_MS, 5 * HOUR_MS).before).toBe("Five hours so far, all of them this past week.");
    expect(libraryPlaytime(3 * HOUR_MS, 0).before).toBe("Three hours so far.");
    expect(libraryPlaytime(30 * MINUTE_MS, 0).before).toBe("Thirty minutes so far.");
    expect(libraryPlaytime(10_000, 0).before).toBe("The clock has just started.");
  });
});

describe("gamePlaytime", () => {
  /** Runtime by hour of the day, `minutes` in each of `hours`. */
  function byHour(minutes: number, ...hours: number[]): number[] {
    return Array.from({ length: 24 }, (_, hour) => (hours.includes(hour) ? minutes * MINUTE_MS : 0));
  }

  it("names a clear habit", () => {
    expect(gamePlaytime({ runtimeMs: 6 * HOUR_MS, sessionsCount: 3 }, byHour(180, 19, 20))).toEqual({
      before: "Six hours across three sessions. You mostly play it ",
      em: "in the evening",
      after: ".",
    });
  });

  it("goes by when the time fell, not when sessions started", () => {
    // Started in the evening but mostly played past 22:00.
    const late = byHour(10, 21).map((ms, hour) => ms + (hour === 23 || hour === 0 ? 2 * HOUR_MS : 0));
    expect(gamePlaytime({ runtimeMs: 4 * HOUR_MS, sessionsCount: 3 }, late).em).toBe(
      "late at night",
    );
  });

  it("skips the habit without a clear one or with few sessions", () => {
    expect(gamePlaytime({ runtimeMs: 2 * HOUR_MS, sessionsCount: 3 }, byHour(40, 9, 14, 20))).toEqual({
      before: "Two hours across three sessions.",
    });
    expect(gamePlaytime({ runtimeMs: HOUR_MS, sessionsCount: 1 }, byHour(60, 20))).toEqual({
      before: "One hour in one session.",
    });
    expect(gamePlaytime({ runtimeMs: 0, sessionsCount: 0 }, [])).toEqual({
      before: "Not played yet. Start it however you usually do and the clock starts on its own.",
    });
  });
});

describe("weekSentence", () => {
  it("sums up the week", () => {
    expect(
      weekSentence({
        sessionsCount: 9,
        runtimeMs: 11 * HOUR_MS + 20 * MINUTE_MS,
        longestDay: "Saturday",
        daysPlayed: 4,
        current: true,
      }),
    ).toEqual({
      before: "Nine sessions, eleven hours and twenty minutes in all. ",
      em: "Saturday",
      after: " was the longest day.",
    });
  });

  it("handles a single day and an empty week", () => {
    expect(
      weekSentence({ sessionsCount: 1, runtimeMs: 40 * MINUTE_MS, longestDay: "Friday", daysPlayed: 1, current: false }),
    ).toEqual({ before: "One session, forty minutes in all. All of it on ", em: "Friday", after: "." });
    expect(
      weekSentence({ sessionsCount: 0, runtimeMs: 0, longestDay: null, daysPlayed: 0, current: false }).before,
    ).toBe("Nothing played that week.");
  });

  it("reads differently from week to week", () => {
    const week = {
      sessionsCount: 9,
      runtimeMs: 10 * HOUR_MS,
      longestDay: "Saturday",
      daysPlayed: 4,
      topTitle: "Balatro",
      topShare: 0.7,
      gamesCount: 3,
      current: false,
    };
    expect(phraseString(weekSentence({ ...week, weekNumber: 1 }))).toBe(
      "Ten hours over nine sessions. Saturday was the longest day.",
    );
    expect(weekSentence({ ...week, weekNumber: 2 })).toEqual({
      before: "Nine sessions, ten hours in all. Most of it went to ",
      em: "Balatro",
      after: ".",
    });
    expect(phraseString(weekSentence({ ...week, weekNumber: 2, topShare: 0.5 }))).toBe(
      "Nine sessions, ten hours in all. Saturday was the longest day.",
    );
    expect(phraseString(weekSentence({ ...week, weekNumber: 3, gamesCount: 1 }))).toBe(
      "Ten hours over nine sessions. All of it in Balatro.",
    );
    expect(phraseString(weekSentence({ ...week, sessionsCount: 1, daysPlayed: 1, weekNumber: 1 }))).toBe(
      "Ten hours in one session. All of it on Saturday.",
    );
  });

  it("counts the end of a session from the week before apart from the week's own", () => {
    const week = { runtimeMs: 5 * HOUR_MS, longestDay: "Monday", daysPlayed: 2, carriedOver: 1, current: false };
    expect(phraseString(weekSentence({ ...week, sessionsCount: 2, weekNumber: 2 }))).toBe(
      "Two sessions and the end of one from the week before, five hours in all. Monday was the longest day.",
    );
    expect(phraseString(weekSentence({ ...week, sessionsCount: 2, weekNumber: 1 }))).toBe(
      "Five hours over two sessions and the end of one from the week before. Monday was the longest day.",
    );
    expect(phraseString(weekSentence({ ...week, sessionsCount: 0, runtimeMs: 110 * MINUTE_MS, daysPlayed: 1 }))).toBe(
      "One hour and fifty minutes from the end of a session that began the week before. All of it on Monday.",
    );
  });
});

describe("lastPlayedLine", () => {
  it("fits the relative day into a sentence", () => {
    const now = Date.now();
    expect(lastPlayedLine(null)).toBe("Not played yet");
    expect(lastPlayedLine(new Date(now - 12 * MINUTE_MS).toISOString())).toBe("Last played 12 min ago");
  });
});

describe("statusSentence", () => {
  it("writes status changes with the playtime so far", () => {
    expect(statusSentence("finished", "Hades II", 42 * HOUR_MS)).toEqual({
      before: "You finished ",
      em: "Hades II",
      after: " after forty-two hours.",
    });
    expect(statusSentence("dropped", "Celeste", 3 * HOUR_MS).after).toBe(" down after three hours.");
    expect(statusSentence("finished", "Celeste", 0).after).toBe(".");
    expect(statusSentence("backlog", "Outer Wilds", 0)).toEqual({ before: "", em: "Outer Wilds", after: " went on your list." });
  });
});

describe("sideBySideSentence", () => {
  const titles: Record<string, string> = { a: "League of Legends", b: "Teamfight Tactics", c: "Balatro", d: "Celeste" };
  const titleOf = (gameId: string) => titles[gameId];
  const group = { ms: 115 * MINUTE_MS, alongside: null, most_at_once: 2 };

  it("puts two games side by side", () => {
    expect(sideBySideSentence({ ...group, game_ids: ["a", "b"] }, titleOf)).toBe(
      "League of Legends and Teamfight Tactics ran side by side for 1 h 55.",
    );
  });

  it("names a game that ran through all of it first", () => {
    expect(sideBySideSentence({ ...group, game_ids: ["a", "b", "c"], alongside: "a", most_at_once: 3 }, titleOf)).toBe(
      "League of Legends ran alongside Teamfight Tactics and Balatro for 1 h 55.",
    );
  });

  it("says how many ran at once when no game ran through all of it", () => {
    expect(sideBySideSentence({ ...group, game_ids: ["a", "b", "c"] }, titleOf)).toBe(
      "League of Legends, Teamfight Tactics and Balatro ran side by side, two at a time, for 1 h 55.",
    );
    expect(sideBySideSentence({ ...group, game_ids: ["a", "b", "c", "d"], most_at_once: 3 }, titleOf)).toBe(
      "League of Legends, Teamfight Tactics, Balatro and Celeste ran side by side, up to three at a time, for 1 h 55.",
    );
  });
});

describe("ledgerSentence", () => {
  const ledger: LedgerReport = {
    key_id: "key",
    began_at: null,
    entries: 10,
    covered_sessions: 4,
    missing_sessions: 0,
    broken: false,
  };

  it("says how much the ledger covers", () => {
    expect(ledgerSentence(ledger)).toBe("It covers all 4 sessions, and none is missing.");
    expect(ledgerSentence({ ...ledger, covered_sessions: 1 })).toBe("It covers your one session, and none is missing.");
    expect(ledgerSentence({ ...ledger, covered_sessions: 0 })).toBe("No sessions yet.");
  });

  it("names what went missing or changed", () => {
    expect(ledgerSentence({ ...ledger, missing_sessions: 1 })).toBe(
      "One session in the ledger is gone from the history without a note that you removed it.",
    );
    expect(ledgerSentence({ ...ledger, missing_sessions: 2 })).toContain("2 sessions in the ledger are gone");
    expect(ledgerSentence({ ...ledger, broken: true })).toContain("changed outside Vaultime");
  });
});
