// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS } from "@/lib/constants";
import { at, event, session } from "@/test/sessions";
import {
  busiestMonthSentence,
  describeSession,
  gamePlaytime,
  lastPlayedLine,
  libraryPlaytime,
  rhythmSentence,
  sessionAmounts,
  sessionTrustNote,
  shapesSentence,
  statusSentence,
  streakSentence,
  weekSentence,
  yearSentence,
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

  it("says when all the time was taken out", () => {
    const discarded = session(at(2026, 9, 29, 20, 12), 165, { runtime_ms: 0, active_ms: 0, idle_ms: 0 });
    expect(describeSession(discarded)).toBe("No play counted");
    expect(describeSession(discarded, "Elden Ring")).toBe("No play counted for Elden Ring");
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

  it("leaves out a session with all its time taken out", () => {
    const played = session(at(2026, 9, 20, 9, 0), 60);
    const discarded = session(at(2026, 9, 21, 20, 0), 60, { runtime_ms: 0, active_ms: 0, idle_ms: 0 });
    expect(gamePlaytime([played, discarded])).toEqual({ before: "One hour in one session." });
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

  it("gives the reason of a correction and of a session added by hand", () => {
    const edited = session(at(2026, 9, 29, 21, 0), 30, { integrity_status: "edited" });
    const corrected = [
      event(edited.id, "corrected", { reason: "left it running", runtime_ms: 30 * MINUTE_MS, previous: { runtime_ms: 5 * HOUR_MS } }),
    ];
    expect(sessionTrustNote(edited, corrected)).toBe("Cut short by you, it had 5 h 00: left it running.");
    const discarded = [
      event(edited.id, "corrected", { reason: "Only the launcher!", runtime_ms: 0, previous: { runtime_ms: HOUR_MS } }),
    ];
    expect(sessionTrustNote(edited, discarded)).toBe("All time taken out by you, it had 1 h 00: Only the launcher!");
    const manual = session(at(2026, 9, 29, 21, 0), 30, { integrity_status: "manual" });
    expect(sessionTrustNote(manual, [event(manual.id, "added_manually", { reason: "On the Steam Deck" })])).toBe(
      "Added by you: On the Steam Deck.",
    );
    expect(sessionTrustNote(manual, [event(manual.id, "added_manually", { reason: "" })])).toBe("Added by you.");
  });
});

describe("lastPlayedLine", () => {
  it("fits the relative day into a sentence", () => {
    const now = Date.now();
    expect(lastPlayedLine(null)).toBe("Not played yet");
    expect(lastPlayedLine(new Date(now - 12 * MINUTE_MS).toISOString())).toBe("Last played 12 min ago");
  });
});

describe("stats sentences", () => {
  const year = { daysPlayed: 90, topMs: 61 * HOUR_MS, gamesCount: 7, year: 2026, current: true };

  it("sums up a year and names the game that led", () => {
    expect(yearSentence({ ...year, playedMs: 212 * HOUR_MS, topTitle: "Elden Ring" })).toEqual({
      before: "Two hundred twelve hours on ninety days. ",
      em: "Elden Ring",
      after: " led with sixty-one hours.",
    });
    expect(yearSentence({ ...year, playedMs: 0, topTitle: null }).before).toBe("Nothing played this year yet.");
    expect(yearSentence({ ...year, playedMs: 0, topTitle: null, current: false }).before).toBe("Nothing played in 2026.");
  });

  it("writes streaks within and across months", () => {
    const march = { days: 12, start: new Date(2026, 2, 3), end: new Date(2026, 2, 14) };
    expect(streakSentence(march, 4, true)).toBe(
      "Your longest streak was twelve days, 3 to 14 March. Right now you are on four days in a row.",
    );
    const turn = { days: 4, start: new Date(2026, 1, 27), end: new Date(2026, 2, 2) };
    expect(streakSentence(turn, 4, false)).toBe("Your longest streak was four days, 27 February to 2 March.");
    expect(streakSentence({ days: 1, start: new Date(2026, 0, 1), end: new Date(2026, 0, 1) }, 1, true)).toBe(
      "No two days in a row yet.",
    );
  });

  it("compares weekdays and weekends per day", () => {
    const clock = Array.from({ length: 7 }, () => Array<number>(24).fill(0));
    // Five weekday evenings of an hour against two weekend afternoons of two hours.
    for (let day = 0; day < 5; day += 1) clock[day][20] = HOUR_MS;
    clock[5][14] = 2 * HOUR_MS;
    clock[6][14] = 2 * HOUR_MS;
    expect(rhythmSentence(clock)?.em).toBe("weekend afternoons");
    expect(rhythmSentence(clock.map((hours) => hours.map(() => 0)))).toBeNull();
  });

  it("names the busiest month and the most common session", () => {
    const months = Array.from({ length: 12 }, (_, index) => ({ activeMs: index === 9 ? 31 * HOUR_MS : HOUR_MS, idleMs: 0 }));
    expect(busiestMonthSentence(months, 2026)).toBe("October was your busiest month, thirty-one hours.");
    expect(shapesSentence({ quick: 2, short: 3, plain: 9, long: 4, marathon: 1 })).toBe(
      "Your most common session was one to two hours.",
    );
    expect(shapesSentence({ quick: 0, short: 0, plain: 0, long: 0, marathon: 0 })).toBeNull();
    expect(busiestMonthSentence(months.map(() => ({ activeMs: 0, idleMs: 0 })), 2026)).toBeNull();
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
