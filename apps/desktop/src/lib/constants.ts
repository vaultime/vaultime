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
export const MINUTES_PER_HOUR = 60;
export const SECONDS_PER_HOUR = 3_600;
export const HOURS_PER_DAY = 24;
export const DAYS_PER_WEEK = 7;

// Settings

/** Keys of the settings table, the Rust core reads the same ones from constants.rs. */
export const SETTING_KEYS = {
  idleThreshold: "idle_threshold_seconds",
  backgroundActive: "treat_background_as_active",
  closeToTray: "close_to_tray",
} as const;

/** Event the core sends when games or covers changed. Same as LIBRARY_CHANGED_EVENT in constants.rs. */
export const LIBRARY_CHANGED_EVENT = "library-changed";

// Tracking

/** How often the UI asks for running sessions. Same as POLL_INTERVAL in constants.rs. */
export const ACTIVE_POLL_MS = 5 * SECOND_MS;

/** How often the live timer redraws between polls. */
export const LIVE_TICK_MS = SECOND_MS;

/** Idle threshold shown until the settings load. Same as DEFAULT_IDLE_THRESHOLD_SECS in constants.rs. */
export const DEFAULT_IDLE_THRESHOLD_SECONDS = 300;

/** Lowest idle threshold the settings accept. Same as MIN_IDLE_THRESHOLD_SECS in constants.rs. */
export const MIN_IDLE_THRESHOLD_SECONDS = 5;

// Cloud

/** Refresh the access token when it expires within this window. */
export const TOKEN_REFRESH_MARGIN_MS = MINUTE_MS;

/** Shortest backup passphrase the app accepts. */
export const MIN_BACKUP_PASSPHRASE_CHARS = 12;

/** Step between byte units, B to KB to MB. */
export const BYTES_PER_KIB = 1_024;

/** Sizes below this many units get one decimal, "4.2 MB" but "42 MB". */
export const SIZE_ONE_DECIMAL_BELOW = 10;

// Library and charts

/** Calendar days that count as recent on the library home, today included. */
export const RECENT_DAYS = 7;

/** Calendar days in the activity charts. */
export const ACTIVITY_CHART_DAYS = 14;

/** Shortest visible bar in a chart, in percent of its height, so small days still show. */
export const CHART_MIN_BAR_PERCENT = 3;

/** Game results in the command palette, before and while typing. */
export const PALETTE_GAMES_IDLE = 5;
export const PALETTE_GAMES_SEARCHING = 8;

/** Titles up to this many characters get the largest hero size, then one step down. */
export const HERO_TITLE_LARGE_MAX_CHARS = 14;
export const HERO_TITLE_MEDIUM_MAX_CHARS = 26;

// Game page

/** Sessions listed before "Show all". */
export const GAME_RECENT_SESSIONS = 8;

/** Notable integrity events listed in a game's event log. */
export const EVENT_LOG_LIMIT = 12;

// Journal

/** Hours between the labels under a day's 24 hour strip. */
export const JOURNAL_TICK_HOURS = 6;

/** Narrowest span on the strip, in percent of the day, so short sessions still show. */
export const JOURNAL_MIN_SPAN_PERCENT = 0.8;

// Time phrases

/** Session lengths that change the words for a session: quick, short, plain, long, marathon. */
export const SESSION_QUICK_MAX_MS = 20 * MINUTE_MS;
export const SESSION_SHORT_MAX_MS = HOUR_MS;
export const SESSION_PLAIN_MAX_MS = 2 * HOUR_MS;
export const SESSION_LONG_MAX_MS = 4 * HOUR_MS;

/** A part of the day counts as a habit after this many sessions and share of runtime. */
export const HABIT_MIN_SESSIONS = 3;
export const HABIT_MIN_SHARE = 0.5;

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
