// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS } from "@/lib/constants";
import { at, event, session } from "@/test/sessions";
import {
  describeSession,
  gamePlaytime,
  lastPlayedLine,
  libraryPlaytime,
  sessionAmounts,
  sessionTrustNote,
  weekSentence,
} from "./sentences";

describe("describeSession", () => {
  it("reads length and time of day", () => {
    expect(describeSession(session(at(2026, 9, 29, 9, 0), 10))).toBe("A quick look in the morning");
    expect(describeSession(session(at(2026, 9, 29, 14, 0), 50))).toBe("A short afternoon session");
    expect(describeSession(session(at(2026, 9, 29, 23, 30), 45))).toBe("A short session late at night");
    expect(describeSession(session(at(2026, 9, 29, 15, 0), 90))).toBe("An afternoon session");
    expect(describeSession(session(at(2026, 9, 29, 23, 16), 112))).toBe("A late night session");
    expect(describeSession(session(at(2026, 9, 29, 20, 12), 165))).toBe("A long evening");
    expect(describeSession(session(at(2026, 9, 29, 18, 0), 300))).toBe("A marathon evening");
  });

  it("names the game when asked", () => {
    const evening = session(at(2026, 9, 29, 20, 12), 165);
    expect(describeSession(evening, "Elden Ring")).toBe("A long evening in Elden Ring");
    const live = { ...evening, ended_at_wall: null };
    expect(describeSession(live)).toBe("Playing now");
    expect(describeSession(live, "Elden Ring")).toBe("Playing Elden Ring now");
  });
});

describe("sessionAmounts", () => {
  it("lists runtime, active and idle time", () => {
    const played = session(at(2026, 9, 29, 22, 10), 72, { active_ms: 65 * MINUTE_MS, idle_ms: 7 * MINUTE_MS });
    expect(sessionAmounts(played)).toBe("1 h 12 in all, 1 h 05 active, 7 min idle");
  });
});

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
  it("names a clear habit", () => {
    const evenings = [20, 21, 22].map((day) => session(at(2026, 9, day, 19, 0), 120));
    expect(gamePlaytime(evenings)).toEqual({
      before: "Six hours across three sessions. You mostly play it ",
      em: "in the evening",
      after: ".",
    });
  });

  it("skips the habit without a clear one", () => {
    const mixed = [session(at(2026, 9, 20, 9, 0), 60), session(at(2026, 9, 21, 20, 0), 60)];
    expect(gamePlaytime(mixed)).toEqual({ before: "Two hours across two sessions." });
    expect(gamePlaytime([])).toEqual({
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
});

describe("sessionTrustNote", () => {
  const flagged = session(at(2026, 9, 29, 21, 0), 60, { integrity_status: "suspicious" });

  it("explains a flag from its event", () => {
    const events = [event(flagged.id, "integrity_flagged", { reason: "wall_clock_step_mismatch" })];
    expect(sessionTrustNote(flagged, events)).toBe("The system clock jumped during it.");
  });

  it("falls back to a general reason", () => {
    expect(sessionTrustNote(flagged, [])).toBe("Timing looked off during it.");
    const recovered = { ...flagged, integrity_status: "recovered" };
    expect(sessionTrustNote(recovered, [])).toBe("Rebuilt after an unclean exit.");
  });

  it("mentions skipped sleep and ignores other sessions", () => {
    const played = session(at(2026, 9, 29, 21, 0), 60);
    const events = [
      event(played.id, "tracking_gap", { wall_gap_ms: 2 * HOUR_MS + 14 * MINUTE_MS }),
      event("someone-else", "integrity_flagged", { reason: "wall_clock_moved_backwards" }),
    ];
    expect(sessionTrustNote(played, events)).toBe("2 h 14 of sleep or pause left out.");
    expect(sessionTrustNote(played, [])).toBeNull();
  });
});

describe("lastPlayedLine", () => {
  it("fits the relative day into a sentence", () => {
    const now = Date.now();
    expect(lastPlayedLine(null)).toBe("Not played yet");
    expect(lastPlayedLine(new Date(now - 12 * MINUTE_MS).toISOString())).toBe("Last played 12 min ago");
  });
});
