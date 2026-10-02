// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Every number that tunes the frontend, in one place. The Rust core keeps its
// own list in src-tauri/src/constants.rs, and values shared with it say so.
// Styling values live in Tailwind classes and the theme in index.css.

// Time units

/** One second in milliseconds. */
export const SECOND_MS = 1_000;
/** One minute in milliseconds. */
export const MINUTE_MS = 60 * SECOND_MS;
/** One hour in milliseconds. */
export const HOUR_MS = 60 * MINUTE_MS;
/** One day in milliseconds, ignoring clock changes. */
export const DAY_MS = 24 * HOUR_MS;
/** Seconds in a minute. */
export const SECONDS_PER_MINUTE = 60;
/** Minutes in an hour. */
export const MINUTES_PER_HOUR = 60;
/** Seconds in an hour. */
export const SECONDS_PER_HOUR = 3_600;
/** Hours in a day. */
export const HOURS_PER_DAY = 24;
/** Days in a week. */
export const DAYS_PER_WEEK = 7;
/** Months in a year. */
export const MONTHS_PER_YEAR = 12;
/** Monday to Friday. The week starts on Monday. */
export const WORKING_DAYS_PER_WEEK = 5;
/** Saturday and Sunday, the last days of the week. */
export const WEEKEND_DAYS_PER_WEEK = DAYS_PER_WEEK - WORKING_DAYS_PER_WEEK;

// Settings

/** Keys of the settings table the page may change. Same as PAGE_SETTINGS in constants.rs. */
export const SETTING_KEYS = {
  /** Same as IDLE_THRESHOLD_SETTING in constants.rs. */
  idleThreshold: "idle_threshold_seconds",
  /** Same as BACKGROUND_ACTIVE_SETTING in constants.rs. */
  backgroundActive: "treat_background_as_active",
  /** Same as CLOSE_TO_TRAY_SETTING in constants.rs. */
  closeToTray: "close_to_tray",
  /** Same as AUTO_BACKUP_ENABLED_SETTING in constants.rs. */
  autoBackup: "auto_backup_enabled",
  /** Same as AUTO_BACKUP_FOLDER_SETTING in constants.rs. */
  autoBackupFolder: "auto_backup_folder",
  /** Same as CLOUD_AUTO_BACKUP_SETTING in constants.rs. */
  cloudAutoBackup: "cloud_auto_backup_enabled",
  /** How the rail sorts the games. Same as RAIL_SORT_SETTING in constants.rs. */
  railSort: "rail_sort",
} as const;

/** Automatic local backups kept. Same as AUTO_BACKUP_KEEP in constants.rs. */
export const AUTO_BACKUP_KEEP = 7;

/** Event the core sends when games or covers changed. Same as LIBRARY_CHANGED_EVENT in constants.rs. */
export const LIBRARY_CHANGED_EVENT = "library-changed";

// Tracking

/** How often the UI asks for running sessions. Same as POLL_INTERVAL in constants.rs. */
export const ACTIVE_POLL_MS = 5 * SECOND_MS;

/** How often the live timer redraws between polls. */
export const LIVE_TICK_MS = SECOND_MS;

/** Idle threshold shown until the settings load. Same as DEFAULT_IDLE_THRESHOLD_SECS in constants.rs. */
export const DEFAULT_IDLE_THRESHOLD_SECS = 300;

/** Lowest idle threshold the settings accept. Same as MIN_IDLE_THRESHOLD_SECS in constants.rs. */
export const MIN_IDLE_THRESHOLD_SECS = 5;
/** Lowest idle time the minus button steps to, in minutes. Typing can go lower. */
export const IDLE_STEP_MIN_MINUTES = 1;
/** Wait after the last change of the idle time before saving it. */
export const IDLE_SAVE_DELAY_MS = 600;

// Cloud

/** Address of the website and the cloud server. Same as CLOUD_API_BASE_URL in constants.rs. */
export const VAULTIME_URL = "https://vaultime.codfishcloud.de";

/** Copyright notice in the rail and in Settings, the holder of the SPDX headers. */
export const COPYRIGHT_NOTICE = "© 2026 Dominik Schwimmbeck";
/** Prefix of invite codes. Same as INVITE_PREFIX in apps/api/src/constants.rs. */
export const INVITE_CODE_PREFIX = "VTLINV";

/** Refresh the access token when it expires within this window. */
export const TOKEN_REFRESH_MARGIN_MS = MINUTE_MS;
/** Age of the newest cloud backup at which the daily automatic one is due. */
export const CLOUD_AUTO_BACKUP_INTERVAL_MS = DAY_MS;
/** How often a signed in app checks whether the daily cloud backup is due. */
export const CLOUD_AUTO_BACKUP_CHECK_MS = HOUR_MS;

/** Shortest backup passphrase the app accepts. */
export const MIN_BACKUP_PASSPHRASE_CHARS = 12;
/** Shortest cloud account password. Same as `MIN_PASSWORD_CHARS` in `apps/api/src/constants.rs`. */
export const MIN_CLOUD_PASSWORD_CHARS = 10;

/** Step between byte units, B to KiB to MiB. */
export const BYTES_PER_KIB = 1_024;

/** Sizes below this many units get one decimal, "4.2 MiB" but "42 MiB". */
export const SIZE_ONE_DECIMAL_BELOW = 10;

/** Browser storage key of the cloud session saved by older builds, read once and removed. */
export const CLOUD_SESSION_STORAGE_KEY = "vaultime.cloud.session";
/** Browser storage key of the id this PC registers with the cloud server. */
export const CLOUD_DEVICE_ID_STORAGE_KEY = "vaultime.cloud.device-id";
/** Browser storage key prefix of the last backup list per account, shown while the server is not reachable. */
export const CLOUD_BACKUPS_CACHE_KEY_PREFIX = "vaultime.cloud.backups.";
/** Random characters in a device id when the webview cannot make a UUID. */
export const FALLBACK_DEVICE_ID_CHARS = 10;

// Library and charts

/** Calendar days that count as recent on the library home, today included. */
export const RECENT_DAYS = 7;

/** The library grid marks a game Suspicious or Recovered only for sessions this many days old or newer. The game page counts all of them. */
export const TRUST_BADGE_RECENT_DAYS = 30;

/** Calendar days in the activity charts. */
export const ACTIVITY_CHART_DAYS = 14;

/** Shortest visible bar in a chart, in percent of its height, so small days still show. */
export const CHART_MIN_BAR_PERCENT = 3;

/** Game results in the command palette before typing. */
export const PALETTE_GAMES_IDLE = 5;
/** Game results in the command palette while typing. */
export const PALETTE_GAMES_SEARCHING = 8;

/** Smallest size a long title in a header shrinks to before it wraps, in CSS pixels. */
export const HERO_TITLE_MIN_FONT_PX = 32;

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

/** Longest session that reads as a quick look. */
export const SESSION_QUICK_MAX_MS = 20 * MINUTE_MS;
/** Longest session that reads as short. */
export const SESSION_SHORT_MAX_MS = HOUR_MS;
/** Longest session that reads as a plain session, longer ones are long. */
export const SESSION_PLAIN_MAX_MS = 2 * HOUR_MS;
/** Longest session that reads as long, longer ones are marathons. */
export const SESSION_LONG_MAX_MS = 4 * HOUR_MS;

/** Time away from a game after which a session reads as a return. */
export const SESSION_RETURN_MIN_MS = 14 * DAY_MS;
/** Earlier sessions a game needs before one can be its longest yet. */
export const SESSION_RECORD_MIN_EARLIER = 5;
/** Shortest session that can be called the longest yet. */
export const SESSION_RECORD_MIN_MS = 2 * HOUR_MS;
/** Share of idle time from which a session reads as left running. */
export const SESSION_LEFT_RUNNING_MIN_SHARE = 0.6;
/** Shortest session that can read as left running. */
export const SESSION_LEFT_RUNNING_MIN_MS = 30 * MINUTE_MS;
/** Play past midnight before a session reads as going into the small hours. */
export const SESSION_PAST_MIDNIGHT_MIN_MS = 30 * MINUTE_MS;
/** Durations under an hour round to this in sentences, longer ones to the half hour. */
export const SESSION_WORDS_MINUTE_STEP_MS = 5 * MINUTE_MS;
/** Share of a week's playtime one game needs to be named in the week sentence. */
export const WEEK_TOP_GAME_MIN_SHARE = 0.6;

/** Sessions in a part of the day before it can count as a habit. */
export const HABIT_MIN_SESSIONS = 3;
/** Share of runtime a part of the day needs to count as a habit. */
export const HABIT_MIN_SHARE = 0.5;

/** Under this many minutes a time reads "Just now". */
export const JUST_NOW_MINUTES = 2;

/** Within this many days a date reads as a weekday, after that as a month. */
export const WEEKDAY_NAME_DAYS = 7;

/** Hours where the parts of the day begin, 24 hour clock. Night wraps past midnight. */
export const DAY_PART_HOURS = { morning: 5, afternoon: 12, evening: 17, night: 22 };

/** Longest note on a session, in characters. Same as SESSION_NOTE_MAX_CHARS in constants.rs. */
export const SESSION_NOTE_MAX_CHARS = 280;

/** Launcher name of Steam. Same as STEAM_SOURCE in constants.rs. */
export const STEAM_LAUNCHER = "steam";

/** Longest session a player can add by hand, in hours. Same as MANUAL_SESSION_MAX in constants.rs. */
export const MANUAL_SESSION_MAX_HOURS = 24;
/** A session added by hand starts this long ago unless the player picks a time. */
export const MANUAL_SESSION_DEFAULT_AGO_MS = 2 * HOUR_MS;

// Stats page

/** Smallest dot of the week clock for an hour with any play, in percent of its cell. */
export const WEEK_CLOCK_MIN_DOT_PERCENT = 22;
/** Games listed on the stats page, the rest are left out. */
export const STATS_TOP_GAMES = 8;
/** Active time per day at which the year heatmap turns one step brighter. */
export const HEATMAP_STEPS_MS = [30 * MINUTE_MS, HOUR_MS, 2 * HOUR_MS, 4 * HOUR_MS];
/** A game shows among those left running from this much runtime in the year. */
export const STATS_IDLE_MIN_RUNTIME_MS = 2 * HOUR_MS;
/** A game shows among those left running from this share of idle runtime. */
export const STATS_IDLE_MIN_SHARE = 0.25;
/** Games shown among those left running. */
export const STATS_IDLE_GAMES = 3;

/** Numbers up to this are written as words in prose, larger ones as digits. */
export const NUMBER_WORDS_MAX = 999;

// Game tints

/** Hue of Vaultime violet in OKLCH, in degrees. The hue of --violet in index.css. */
export const BRAND_HUE_DEG = 293;

/** Lightness and chroma of each tint role, the hue comes from the game. */
export const TINT_LEVELS = {
  fill: { lightness: 0.3, chroma: 0.06 },
  edge: { lightness: 0.38, chroma: 0.07 },
  ink: { lightness: 0.93, chroma: 0.045 },
  soft: { lightness: 0.85, chroma: 0.05 },
  muted: { lightness: 0.77, chroma: 0.06 },
  wash: { lightness: 0.19, chroma: 0.035 },
};

/**
 * Lightness and chroma of the marks that tell games apart, as in the journal.
 * Stays inside sRGB at every hue, so no hue gets clipped.
 */
export const MARK_LEVELS = { lightness: 0.74, chroma: 0.125 };

/** Games shown side by side keep their hues at least this far apart, in degrees. */
export const MARK_MIN_HUE_GAP_DEG = 50;

/** Covers are scaled down to this many pixels square before reading colors. */
export const TINT_SAMPLE_PX = 32;

/** Pixels more transparent than this alpha byte are skipped. */
export const TINT_MIN_ALPHA = 128;

/** Pixels darker than this OKLab lightness are skipped as black. */
export const TINT_MIN_LIGHTNESS = 0.12;
/** Pixels lighter than this OKLab lightness are skipped as white. */
export const TINT_MAX_LIGHTNESS = 0.96;

/** Mean OKLab chroma of a typical colorful cover, it maps to full tint strength. */
export const TINT_TYPICAL_CHROMA = 0.09;

/** How far a tint may fade towards grey. */
export const TINT_MIN_STRENGTH = 0.2;
/** How far a tint may go past the default colorfulness. */
export const TINT_MAX_STRENGTH = 1.2;
