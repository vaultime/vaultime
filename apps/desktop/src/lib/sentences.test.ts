// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS } from "@/lib/constants";
import { at, event, session } from "@/test/sessions";
import type { Session } from "@/lib/types";
import {
  busiestMonthSentence,
  describeSession,
  gamePlaytime,
  lastPlayedLine,
  libraryPlaytime,
  phraseString,
  rhythmSentence,
  sessionAmounts,
  type SessionContext,
  sessionTrustNote,
  shapesSentence,
  statusSentence,
  streakSentence,
  weekSentence,
  yearSentence,
} from "./sentences";

/** Every way a session can be put, found by giving it other ids. */
function wordings(base: Session, context: SessionContext = {}): Set<string> {
  return new Set(
    Array.from({ length: 40 }, (_, index) => phraseString(describeSession({ ...base, id: `variant-${index}` }, context))),
  );
}

describe("describeSession", () => {
  it("reads length and time of day", () => {
    expect(wordings(session(at(2026, 9, 29, 9, 0), 10))).toEqual(
      new Set(["A quick look in the morning", "A few minutes in the morning", "A brief morning visit"]),
    );
    expect(wordings(session(at(2026, 9, 29, 14, 0), 50))).toEqual(
      new Set(["A short afternoon session", "Fifty minutes in the afternoon", "An afternoon round"]),
    );
    expect(wordings(session(at(2026, 9, 29, 23, 30), 45))).toEqual(
      new Set(["A short session late at night", "Forty-five minutes late at night", "A late night round"]),
    );
    expect(wordings(session(at(2026, 9, 29, 15, 0), 90))).toEqual(
      new Set(["An afternoon session", "An hour and a half in the afternoon", "An afternoon of play"]),
    );
    expect(wordings(session(at(2026, 9, 29, 20, 12), 150))).toEqual(
      new Set(["A long evening", "Most of the evening", "Two and a half hours in the evening"]),
    );
    expect(wordings(session(at(2026, 9, 29, 18, 0), 300))).toEqual(
      new Set(["A marathon evening", "A whole evening of play", "Five hours in one go"]),
    );
  });

  it("names the weekend and the small hours", () => {
    expect(wordings(session(at(2026, 10, 3, 15, 0), 90))).toContain("A Saturday afternoon of play");
    expect(wordings(session(at(2026, 10, 4, 2, 0), 90))).toEqual(
      new Set(["A late night session", "An hour and a half in the small hours", "A Saturday night of play"]),
    );
  });

  it("keeps the same words for the same session", () => {
    const evening = session(at(2026, 9, 29, 20, 12), 150);
    expect(describeSession(evening)).toEqual(describeSession({ ...evening }));
  });

  it("puts the game in italics", () => {
    const evening = session(at(2026, 9, 29, 20, 12), 150);
    expect(wordings(evening, { gameTitle: "Elden Ring" })).toEqual(
      new Set(["A long evening in Elden Ring", "Most of the evening in Elden Ring", "Two and a half hours deep in Elden Ring"]),
    );
    expect(describeSession(evening, { gameTitle: "Elden Ring" }).em).toBe("Elden Ring");
    const live = { ...evening, ended_at_wall: null };
    expect(describeSession(live)).toEqual({ before: "Playing now" });
    expect(describeSession(live, { gameTitle: "Elden Ring" })).toEqual({ before: "Playing ", em: "Elden Ring", after: " now" });
  });

  it("says when all the time was taken out", () => {
    const discarded = session(at(2026, 9, 29, 20, 12), 165, { runtime_ms: 0, active_ms: 0, idle_ms: 0 });
    expect(describeSession(discarded)).toEqual({ before: "No play counted" });
    expect(phraseString(describeSession(discarded, { gameTitle: "Elden Ring" }))).toBe("No play counted for Elden Ring");
  });

  describe("with the other sessions of the game", () => {
    const gameTitle = "Hades II";
    const earlier = session(at(2026, 9, 25, 20, 0), 60);

    it("notes a first session, unless there was play before Vaultime", () => {
      const first = session(at(2026, 9, 29, 20, 0), 15);
      expect(wordings(first, { gameTitle, gameSessions: [first] })).toEqual(
        new Set(["A first look at Hades II", "First steps in Hades II"]),
      );
      const long = session(at(2026, 9, 29, 20, 0), 150);
      expect(wordings(long, { gameTitle, gameSessions: [long] })).toEqual(new Set(["A first evening in Hades II"]));
      expect(wordings(first, { gameTitle, gameSessions: [first], earlierMs: 10 * HOUR_MS })).toContain(
        "A quick look at Hades II in the evening",
      );
    });

    it("notes a return after a break", () => {
      const back = session(at(2026, 9, 29, 20, 0), 60);
      const away = (start: string) => wordings(back, { gameTitle, gameSessions: [session(start, 60), back] });
      expect(away(at(2026, 9, 10, 20, 0))).toEqual(
        new Set(["Back to Hades II after two weeks", "A return to Hades II after two weeks"]),
      );
      expect(away(at(2026, 6, 15, 20, 0))).toContain("Back to Hades II after three months");
      expect(away(at(2025, 8, 1, 20, 0))).toContain("Back to Hades II after over a year");
    });

    it("notes a game left running, through the evening only when it was long", () => {
      const open = session(at(2026, 9, 29, 19, 0), 150, { active_ms: 20 * MINUTE_MS, idle_ms: 130 * MINUTE_MS });
      expect(wordings(open, { gameTitle, gameSessions: [earlier, open] })).toEqual(
        new Set(["Hades II stayed open through the evening", "Hades II ran mostly on its own"]),
      );
      const short = session(at(2026, 9, 29, 23, 0), 30, { active_ms: 10 * MINUTE_MS, idle_ms: 20 * MINUTE_MS });
      expect(wordings(short, { gameTitle, gameSessions: [earlier, short] })).toEqual(
        new Set(["Hades II ran mostly on its own"]),
      );
    });

    it("notes the longest session yet", () => {
      const before = [21, 22, 23, 24, 25].map((day) => session(at(2026, 9, day, 20, 0), 60));
      const long = session(at(2026, 9, 29, 19, 0), 150);
      expect(wordings(long, { gameTitle, gameSessions: [...before, long] })).toEqual(
        new Set(["Your longest session of Hades II yet", "Two and a half hours of Hades II, your longest yet"]),
      );
      expect(wordings(long, { gameTitle, gameSessions: [...before.slice(1), long] })).toContain(
        "A long evening in Hades II",
      );
      expect(wordings(long, { gameTitle, gameSessions: [...before, long], earlierMs: 300 * HOUR_MS })).toContain(
        "A long evening in Hades II",
      );
    });

    it("notes play past midnight and another round on the same day", () => {
      const late = session(at(2026, 9, 29, 23, 0), 90);
      expect(wordings(late, { gameTitle, gameSessions: [earlier, late] })).toEqual(
        new Set(["Into the small hours with Hades II", "Past midnight in Hades II"]),
      );
      // Slept through: started late, the clock says morning, but only 35 minutes counted.
      const slept = session(at(2026, 9, 29, 23, 50), 35, { ended_at_wall: at(2026, 9, 30, 9, 30) });
      expect(wordings(slept, { gameTitle, gameSessions: [earlier, slept] })).not.toContain("Past midnight in Hades II");
      const afternoon = session(at(2026, 9, 29, 14, 0), 60);
      const again = session(at(2026, 9, 29, 20, 0), 40);
      expect(wordings(again, { gameTitle, gameSessions: [earlier, afternoon, again] })).toEqual(
        new Set(["Another round of Hades II", "Back to Hades II for another go"]),
      );
    });
  });
});

describe("sessionAmounts", () => {
  it("lists runtime, active and idle time", () => {
    const played = session(at(2026, 9, 29, 22, 10), 72, { active_ms: 65 * MINUTE_MS, idle_ms: 7 * MINUTE_MS });
    expect(sessionAmounts(played)).toBe("1 h 12 in all, 1 h 05 active, 7 min idle");
  });

  it("shortens a session that was all active or all idle", () => {
    expect(sessionAmounts(session(at(2026, 9, 29, 22, 10), 72))).toBe("1 h 12, all of it active");
    const idle = session(at(2026, 9, 29, 22, 10), 72, { active_ms: 0, idle_ms: 72 * MINUTE_MS });
    expect(sessionAmounts(idle)).toBe("1 h 12, all of it idle");
    const discarded = session(at(2026, 9, 29, 22, 10), 72, { runtime_ms: 0, active_ms: 0, idle_ms: 0 });
    expect(sessionAmounts(discarded)).toBe("0 min in all, 0 min active, 0 min idle");
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
});

describe("sessionTrustNote", () => {
  const flagged = session(at(2026, 9, 29, 21, 0), 60, { integrity_status: "suspicious" });

  it("explains a flag from its event", () => {
    const events = [event(flagged.id, "integrity_flagged", { reason: "wall_clock_step_mismatch" })];
    expect(sessionTrustNote(flagged, events)).toBe("The system clock jumped during it.");
  });

  it("falls back to a general reason", () => {
    expect(sessionTrustNote(flagged, [])).toBe("The clock jumped or the record was changed outside Vaultime.");
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
