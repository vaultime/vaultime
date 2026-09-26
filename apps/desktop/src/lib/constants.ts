// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

// Every number that tunes the frontend, in one place. The Rust core keeps its
// own list in src-tauri/src/constants.rs, and values shared with it say so.
// Styling values live in Tailwind classes and the theme in index.css.

// Time units

export const SECOND_MS = 1_000;
export const MINUTE_MS = 60 * SECOND_MS;
export const HOUR_MS = 60 * MINUTE_MS;
export const DAY_MS = 24 * HOUR_MS;
export const SECONDS_PER_MINUTE = 60;
export const SECONDS_PER_HOUR = 3_600;

// Tracking

/** How often the UI asks for running sessions. Same as POLL_INTERVAL in constants.rs. */
export const ACTIVE_POLL_MS = 5 * SECOND_MS;

/** How often the live timer redraws between polls. */
export const LIVE_TICK_MS = SECOND_MS;

/** Idle threshold shown until the settings load. Same as DEFAULT_IDLE_THRESHOLD_SECS in constants.rs. */
export const DEFAULT_IDLE_THRESHOLD_SECONDS = 300;

// Cloud

/** Refresh the access token when it expires within this window. */
export const TOKEN_REFRESH_MARGIN_MS = MINUTE_MS;

/** Step between byte units, B to KB to MB. */
export const BYTES_PER_KIB = 1_024;

// Library and charts

/** Calendar days that count as recent on the library home, today included. */
export const RECENT_DAYS = 7;

/** Calendar days in the activity charts. */
export const ACTIVITY_CHART_DAYS = 14;

/** Game results in the command palette, before and while typing. */
export const PALETTE_GAMES_IDLE = 5;
export const PALETTE_GAMES_SEARCHING = 8;

/** Titles up to this many characters get the largest hero size, then one step down. */
export const HERO_TITLE_LARGE_MAX_CHARS = 14;
export const HERO_TITLE_MEDIUM_MAX_CHARS = 26;

// Time phrases

/** Under this many minutes a time reads "Just now". */
export const JUST_NOW_MINUTES = 2;

/** Within this many days a date reads as a weekday, after that as a month. */
export const WEEKDAY_NAME_DAYS = 7;

/** Hours where the parts of the day begin, 24 hour clock. Night wraps past midnight. */
export const DAY_PART_HOURS = { morning: 5, afternoon: 12, evening: 17, night: 22 };

/** Numbers up to this are written as words in prose, larger ones as digits. */
export const NUMBER_WORDS_MAX = 999;

// Game tints

/** Hue of Vaultime violet in OKLCH. */
export const BRAND_HUE = 293;

/** Lightness and chroma of each tint role, the hue comes from the game. */
export const TINT_LEVELS = {
  fill: { lightness: 0.3, chroma: 0.06 },
  edge: { lightness: 0.38, chroma: 0.07 },
  ink: { lightness: 0.93, chroma: 0.045 },
  soft: { lightness: 0.85, chroma: 0.05 },
  muted: { lightness: 0.77, chroma: 0.06 },
  wash: { lightness: 0.19, chroma: 0.035 },
};

/** Covers are scaled down to this many pixels square before reading colors. */
export const TINT_SAMPLE_PX = 32;

/** Pixels more transparent than this alpha byte are skipped. */
export const TINT_MIN_ALPHA = 128;

/** Pixels outside this OKLab lightness range are skipped as black or white. */
export const TINT_MIN_LIGHTNESS = 0.12;
export const TINT_MAX_LIGHTNESS = 0.96;

/** Mean OKLab chroma of a typical colorful cover, it maps to full tint strength. */
export const TINT_TYPICAL_CHROMA = 0.09;

/** How far a tint may fade towards grey or go past the default colorfulness. */
export const TINT_MIN_STRENGTH = 0.2;
export const TINT_MAX_STRENGTH = 1.2;
