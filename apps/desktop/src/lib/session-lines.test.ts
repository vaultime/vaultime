// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS } from "@/lib/constants";
import type { Session } from "@/lib/types";
import { phraseString } from "@/test/phrases";
import { at, event, session } from "@/test/sessions";
import {
  afterMidnightLead,
  beganWords,
  continuedAmounts,
  continuedPhrase,
  type ContinuedPhrase,
  describeSession,
  sessionAmounts,
  type SessionContext,
  sessionTrustNote,
} from "./session-lines";

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

describe("play past midnight", () => {
  // Saturday 22:40 to Sunday 02:15.
  const late = session(at(2026, 10, 17, 22, 40), 215, { id: "late" });
  const sunday = new Date(2026, 9, 18);

  it("names when a session that went on began", () => {
    expect(beganWords(new Date(2026, 9, 17, 22, 40), sunday)).toBe("Saturday night");
    expect(beganWords(new Date(2026, 9, 17, 19, 0), sunday)).toBe("Saturday evening");
    expect(beganWords(new Date(2026, 9, 17, 2, 0), sunday)).toBe("early Saturday");
    expect(beganWords(new Date(2026, 9, 3, 20, 0), sunday)).toBe("3 October");
  });

  it("puts the next day's part as the same session going on", () => {
    const lines = new Set(
      Array.from({ length: 20 }, (_, index) => {
        const phrase = continuedPhrase({ ...late, id: `late-${index}` }, "Elden Ring", sunday);
        return `${phrase.before}${phrase.em}${phrase.middle}[${phrase.link}]${phrase.after}`;
      }),
    );
    expect(lines).toEqual(
      new Set(["Still in Elden Ring from [Saturday night]", "Elden Ring, carried on from [Saturday night]"]),
    );
    expect(continuedPhrase(late, "Elden Ring", sunday)).toEqual(continuedPhrase(late, "Elden Ring", sunday));
  });

  it("says when it ran all day or still runs", () => {
    const text = (phrase: ContinuedPhrase) =>
      `${phrase.before}${phrase.em}${phrase.middle}[${phrase.link}]${phrase.after}`;
    expect([
      "Still in Stardew Valley from [Saturday night], all through the day",
      "Stardew Valley ran on all day, open since [Saturday night]",
    ]).toContain(text(continuedPhrase(late, "Stardew Valley", sunday, { throughDay: true })));
    expect([
      "Still playing Elden Ring from [Saturday night]",
      "Elden Ring has been running since [Saturday night]",
    ]).toContain(text(continuedPhrase({ ...late, ended_at_wall: null }, "Elden Ring", sunday)));
  });

  it("gives the part after midnight and the part on a later day with the whole", () => {
    // Amounts never break across lines.
    const plain = (text: string) => text.replaceAll(" ", " ");
    expect(afterMidnightLead(135 * MINUTE_MS)).toBe("2 h 15 of it after midnight, counted on ");
    expect(afterMidnightLead(20 * 1000)).toBe("Under a minute of it after midnight, counted on ");
    expect(plain(continuedAmounts(135 * MINUTE_MS, 215 * MINUTE_MS, false))).toBe(
      "2 h 15 of its 3 h 35 count on this day.",
    );
    expect(plain(continuedAmounts(20 * MINUTE_MS, 70 * MINUTE_MS, true))).toBe(
      "20 min of its 1 h 10 so far count on this day.",
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
    expect(sessionTrustNote(edited, corrected)).toBe("Cut short by you from 5 h 00 (“left it running”).");
    const discarded = [
      event(edited.id, "corrected", { reason: "Only the launcher!", runtime_ms: 0, previous: { runtime_ms: HOUR_MS } }),
    ];
    expect(sessionTrustNote(edited, discarded)).toBe("All 1 h 00 taken out by you (“Only the launcher!”).");
    const manual = session(at(2026, 9, 29, 21, 0), 30, { integrity_status: "manual" });
    expect(sessionTrustNote(manual, [event(manual.id, "added_manually", { reason: "On the Steam Deck" })])).toBe(
      "Added by you (“On the Steam Deck”).",
    );
    expect(sessionTrustNote(manual, [event(manual.id, "added_manually", { reason: "" })])).toBe("Added by you.");
    // A short reason never reads like part of a time.
    const short = [event(edited.id, "corrected", { reason: "123", runtime_ms: HOUR_MS, previous: { runtime_ms: 2 * HOUR_MS } })];
    expect(sessionTrustNote(edited, short)).toBe("Cut short by you from 2 h 00 (“123”).");
  });
});
