// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { describe, expect, it } from "vitest";
import { HOUR_MS, MINUTE_MS, SECOND_MS } from "@/lib/constants";
import { at } from "@/test/sessions";
import {
  clockPercent,
  formatClock,
  formatDayPart,
  formatHoursMinutes,
  formatHoursShort,
  formatRelativeDay,
  formatSessionStart,
  isoWeekNumber,
  parseVaultimeDate,
  startOfWeek,
  UI_LOCALE,
} from "./time";

// Tuesday evening, local time.
const now = new Date(2026, 8, 29, 21, 30);

describe("test setup", () => {
  it("runs in the time zone of the project", () => {
    const canonical = new Intl.DateTimeFormat("en", { timeZone: import.meta.env.TZ }).resolvedOptions().timeZone;
    expect(Intl.DateTimeFormat().resolvedOptions().timeZone).toBe(canonical);
  });
});

describe("UI_LOCALE", () => {
  it("keeps English words with the user's region", () => {
    expect(UI_LOCALE).toBe("en-DE");
  });
});

describe("parseVaultimeDate", () => {
  it("reads timestamps without a zone as UTC", () => {
    expect(parseVaultimeDate("2026-09-29T20:00:00").toISOString()).toBe("2026-09-29T20:00:00.000Z");
  });

  it("reads a bare date as the local calendar day", () => {
    const day = parseVaultimeDate("2026-09-29");
    expect([day.getFullYear(), day.getMonth(), day.getDate(), day.getHours()]).toEqual([2026, 8, 29, 0]);
  });

  it("keeps an explicit offset", () => {
    expect(parseVaultimeDate("2026-09-29T22:00:00+02:00").toISOString()).toBe("2026-09-29T20:00:00.000Z");
  });
});

describe("durations", () => {
  it("formats hours and minutes", () => {
    expect(formatHoursMinutes(38 * MINUTE_MS)).toBe("38 min");
    expect(formatHoursMinutes(4 * HOUR_MS + 5 * MINUTE_MS)).toBe("4 h 05");
  });

  it("rounds down to whole hours for lists", () => {
    expect(formatHoursShort(40 * MINUTE_MS)).toBe("40 min");
    expect(formatHoursShort(142 * HOUR_MS + 59 * MINUTE_MS)).toBe("142 h");
  });

  it("formats a stopwatch", () => {
    expect(formatClock(HOUR_MS + 24 * MINUTE_MS + 10 * SECOND_MS)).toBe("01:24:10");
    expect(formatClock(-5)).toBe("00:00:00");
  });
});

describe("formatRelativeDay", () => {
  it("counts minutes and hours on the same day", () => {
    expect(formatRelativeDay(at(2026, 9, 29, 21, 29), now)).toBe("Just now");
    expect(formatRelativeDay(at(2026, 9, 29, 21, 18), now)).toBe("12 min ago");
    expect(formatRelativeDay(at(2026, 9, 29, 20, 20), now)).toBe("An hour ago");
    expect(formatRelativeDay(at(2026, 9, 29, 8, 0), now)).toBe("13 hours ago");
  });

  it("names recent days and older months", () => {
    expect(formatRelativeDay(at(2026, 9, 28, 23, 0), now)).toBe("Yesterday");
    expect(formatRelativeDay(at(2026, 9, 26, 12, 0), now)).toBe("Saturday");
    expect(formatRelativeDay(at(2026, 8, 3, 12, 0), now)).toBe("In August");
    expect(formatRelativeDay(at(2025, 8, 3, 12, 0), now)).toBe("In August 2025");
  });
});

describe("formatDayPart", () => {
  it("names the part of the day", () => {
    expect(formatDayPart(at(2026, 9, 29, 9, 0), now)).toBe("This morning");
    expect(formatDayPart(at(2026, 9, 28, 19, 0), now)).toBe("Yesterday evening");
    expect(formatDayPart(at(2026, 9, 26, 15, 0), now)).toBe("Saturday afternoon");
  });

  it("counts play after midnight to the night before", () => {
    expect(formatDayPart(at(2026, 9, 29, 1, 30), now)).toBe("Last night");
    expect(formatDayPart(at(2026, 9, 29, 23, 0), new Date(2026, 8, 29, 23, 30))).toBe("Tonight");
  });
});

describe("formatSessionStart", () => {
  it("uses the day and a 24 hour clock", () => {
    expect(formatSessionStart(at(2026, 9, 29, 20, 5), now)).toBe("Today 20:05");
    expect(formatSessionStart(at(2026, 9, 28, 22, 10), now)).toBe("Yesterday 22:10");
    expect(formatSessionStart(at(2026, 9, 16, 21, 40), now)).toMatch(/^16 Sept? 21:40$/);
  });
});

describe("weeks", () => {
  it("starts on Monday", () => {
    expect(startOfWeek(new Date(2026, 9, 4, 12, 0))).toEqual(new Date(2026, 8, 28));
    expect(startOfWeek(new Date(2026, 8, 28, 0, 0))).toEqual(new Date(2026, 8, 28));
  });

  it("numbers weeks like ISO 8601", () => {
    expect(isoWeekNumber(new Date(2026, 8, 29))).toBe(40);
    expect(isoWeekNumber(new Date(2026, 0, 1))).toBe(1);
    // 1 January 2027 is a Friday, so it still belongs to the last week of 2026.
    expect(isoWeekNumber(new Date(2027, 0, 1))).toBe(53);
    expect(isoWeekNumber(new Date(2027, 0, 4))).toBe(1);
  });
});

// Daylight saving ends on 25 October 2026 in Berlin, on 1 November in Los
// Angeles, and starts on 27 September in Auckland. Other zones do not switch.
describe("daylight saving", () => {
  it("keeps whole days across a switch", () => {
    expect(formatRelativeDay(at(2026, 10, 25, 12, 0), new Date(2026, 9, 26, 10, 0))).toBe("Yesterday");
    expect(formatRelativeDay(at(2026, 10, 31, 12, 0), new Date(2026, 10, 2, 10, 0))).toBe("Saturday");
    expect(startOfWeek(new Date(2026, 9, 28, 12, 0))).toEqual(new Date(2026, 9, 26));
    expect(isoWeekNumber(new Date(2026, 9, 25, 23, 0))).toBe(43);
    expect(isoWeekNumber(new Date(2026, 9, 26, 0, 30))).toBe(44);
  });

  it("places the strip by clock time on a 25 hour day", () => {
    const day = new Date(2026, 9, 25);
    expect(clockPercent(new Date(2026, 9, 25, 6, 0), day)).toBe(25);
    expect(clockPercent(new Date(2026, 9, 25, 23, 0), day)).toBeCloseTo((23 / 24) * 100);
    expect(clockPercent(new Date(2026, 9, 24, 23, 0), day)).toBe(0);
    expect(clockPercent(new Date(2026, 9, 26, 0, 30), day)).toBe(100);
  });
});
