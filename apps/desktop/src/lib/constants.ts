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
  /** Dark, light or the system's mode. Same as APPEARANCE_MODE_SETTING in constants.rs. */
  appearanceMode: "appearance_mode",
  /** The ground tone. Same as APPEARANCE_GROUND_SETTING in constants.rs. */
  appearanceGround: "appearance_ground",
  /** The accent color. Same as APPEARANCE_ACCENT_SETTING in constants.rs. */
  appearanceAccent: "appearance_accent",
  /** How far the background picture is dimmed. Same as BACKGROUND_DIM_SETTING in constants.rs. */
  backgroundDim: "background_dim",
  /** How far the background picture is blurred. Same as BACKGROUND_BLUR_SETTING in constants.rs. */
  backgroundBlur: "background_blur",
} as const;

/** Automatic local backups kept. Same as AUTO_BACKUP_KEEP in constants.rs. */
export const AUTO_BACKUP_KEEP = 7;

/** Event the core sends when games or covers changed. Same as LIBRARY_CHANGED_EVENT in constants.rs. */
export const LIBRARY_CHANGED_EVENT = "library-changed";

// Updates

/** How often a running app looks for a new version. The first look is right at start. */
export const UPDATE_CHECK_INTERVAL_MS = 6 * HOUR_MS;

// Tracking

/** How often the UI asks for running sessions. Same as POLL_INTERVAL in constants.rs. */
export const ACTIVE_POLL_MS = 5 * SECOND_MS;

/** How often the live timer redraws between polls. */
export const LIVE_TICK_MS = SECOND_MS;

/** Idle threshold shown until the settings load. Same as DEFAULT_IDLE_THRESHOLD_SECS in constants.rs. */
export const DEFAULT_IDLE_THRESHOLD_SECS = 300;

/** Lowest idle threshold the settings accept. Same as MIN_IDLE_THRESHOLD_SECS in constants.rs. */
export const MIN_IDLE_THRESHOLD_SECS = 5;
/** Highest idle threshold the field takes, a day. Far past it the core can no longer read the value. */
export const MAX_IDLE_THRESHOLD_SECS = DAY_MS / SECOND_MS;
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

/** Days in the game page's daily history, today included. */
export const GAME_HISTORY_DAYS = 30;
/** Months in the game page's monthly history, this month included. */
export const GAME_HISTORY_MONTHS = 12;

/** Shortest visible bar in a chart, in percent of its height, so small days still show. */
export const CHART_MIN_BAR_PERCENT = 3;

/** How often charts read the totals again while a game runs. */
export const LIVE_TOTALS_REFRESH_MS = MINUTE_MS;

/** Games a chart's hover card names before it sums up the rest. */
export const PLAY_CARD_GAMES = 5;

/** Game results in the command palette before typing. */
export const PALETTE_GAMES_IDLE = 5;
/** Game results in the command palette while typing. */
export const PALETTE_GAMES_SEARCHING = 8;

/** Smallest size a long title in a header shrinks to before it wraps, in CSS pixels. */
export const HERO_TITLE_MIN_FONT_PX = 32;

// Game page

/** Launchers whose games also count by any game program in their install folder. Same as FOLDER_MATCH_LAUNCHERS in constants.rs. */
export const FOLDER_MATCH_LAUNCHERS = [
  "steam",
  "epic",
  "gog",
  "heroic",
  "battlenet",
  "riot",
  "hoyoplay",
  "ubisoft",
  "ea",
  "rockstar",
  "xbox",
  "amazon",
  "itch",
];

/** Share of a game's time other games must run beside it before its page suggests stepping aside. */
export const STEPS_ASIDE_HINT_MIN_SHARE = 0.5;
/** Time other games must run beside a game before its page suggests stepping aside. */
export const STEPS_ASIDE_HINT_MIN_MS = HOUR_MS;

/** Sessions listed before "Show all". */
export const GAME_RECENT_SESSIONS = 8;

/** Notable integrity events listed in a game's event log. */
export const EVENT_LOG_LIMIT = 12;

// Cover crop

/**
 * Width over height of a cover. Same as `COVER_WIDTH_PX / COVER_HEIGHT_PX` in
 * `constants.rs`.
 */
export const COVER_ASPECT = 3 / 4;
/** Width of a cover in pixels. Same as `COVER_WIDTH_PX` in `constants.rs`. */
export const COVER_WIDTH_PX = 720;
/** Above this many cover pixels per image pixel the crop dialog warns that the cover may look soft. */
export const CROP_SOFT_SCALE = 2;

/**
 * How far the crop dialog zooms out, as a share of the zoom that fits the
 * whole image, so a logo can keep a margin. `CROP_MAX_FRAME_OF_FIT` in
 * `constants.rs` allows more.
 */
export const CROP_MIN_ZOOM_OF_FIT = 0.5;
/** How far the crop dialog zooms in, as a multiple of the zoom that fills the cover. */
export const CROP_MAX_ZOOM_OF_FILL = 4;

/**
 * Images at least this wide for their height open fitted whole, so a wide
 * logo is not cut. Taller ones open filling the cover.
 */
export const CROP_FIT_MIN_ASPECT = 0.9;

/** One arrow key moves the image by this share of the frame. */
export const CROP_KEY_PAN_SHARE = 0.02;
/** One plus or minus key, or one wheel notch, zooms by this factor. */
export const CROP_ZOOM_STEP = 1.1;
/** Wheel distance in pixels that counts as one notch. */
export const CROP_WHEEL_NOTCH_PX = 100;
/** Crops closer than this in every edge, in fractions of the image, are the same crop. */
export const CROP_SAME_TOLERANCE = 1e-6;
/** Slider steps between the smallest and largest zoom. */
export const CROP_SLIDER_STEPS = 200;

/**
 * Source of artwork the player added, as the core names it. Everything else
 * Vaultime found. Same as `PLAYER_ARTWORK_SOURCE` in constants.rs.
 */
export const PLAYER_ARTWORK_SOURCE = "user_picked";

/** File types the cover and background pickers offer. The core reads all of them. */
export const ARTWORK_EXTENSIONS = [
  "png",
  "jpg",
  "jpeg",
  "webp",
  "gif",
  "bmp",
  "ico",
  "tif",
  "tiff",
  "tga",
  "qoi",
  "pbm",
  "pgm",
  "ppm",
  "pnm",
  "pam",
  "svg",
];
/**
 * The same types for the file dialog. GTK on Linux matches extensions with
 * their case, so the upper case spelling is offered too.
 */
export const ARTWORK_DIALOG_EXTENSIONS = [
  ...ARTWORK_EXTENSIONS,
  ...ARTWORK_EXTENSIONS.map((extension) => extension.toUpperCase()),
];

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

/**
 * Chroma of each tint role and its lightness in dark and light mode, the hue
 * comes from the game. Light mode turns the dark field into a pale one with
 * dark ink.
 */
export const TINT_LEVELS = {
  fill: { chroma: 0.06, dark: 0.3, light: 0.9 },
  edge: { chroma: 0.07, dark: 0.38, light: 0.82 },
  ink: { chroma: 0.045, dark: 0.93, light: 0.24 },
  soft: { chroma: 0.05, dark: 0.85, light: 0.34 },
  muted: { chroma: 0.06, dark: 0.77, light: 0.45 },
  wash: { chroma: 0.035, dark: 0.19, light: 0.965 },
};

/**
 * Lightness of the marks that tell games apart, as in the journal, in dark
 * and light mode, and the chroma of marks for games without artwork.
 */
export const MARK_LEVELS = { lightness: { dark: 0.72, light: 0.53 }, chroma: 0.125 };

/** Marks are this much more colorful than the art, which reads as dull at the size of a bar. */
export const MARK_CHROMA_BOOST = 1.5;
/** Marks take at least this chroma, so a muted cover still reads as a color. */
export const MARK_MIN_CHROMA = 0.1;
/** Marks take at most this chroma, so a vivid cover does not glare. */
export const MARK_MAX_CHROMA = 0.21;

/**
 * OKLCH lightness of the grey marks for black, white and grey artwork in dark
 * and light mode, the first game of the week takes the first.
 */
export const MARK_NEUTRAL_LIGHTNESS = { dark: [0.84, 0.64, 0.95], light: [0.42, 0.6, 0.24] };

/** Chroma of the tinted greys that black, white and grey art gets once the grey steps run out. */
export const MARK_TINTED_GREY_CHROMA = 0.04;

/** Games shown side by side keep their hues at least this far apart, in degrees. */
export const MARK_MIN_HUE_GAP_DEG = 30;

/** No sRGB color has more OKLCH chroma than this. */
export const GAMUT_SEARCH_MAX_CHROMA = 0.4;
/** Halvings when searching the most chroma inside sRGB, enough for three decimals. */
export const GAMUT_SEARCH_STEPS = 12;

/** Covers are scaled down to this many pixels square before reading colors. */
export const TINT_SAMPLE_PX = 32;

/** Pixels more transparent than this alpha byte are skipped. */
export const TINT_MIN_ALPHA = 128;

/** Pixels darker than this OKLab lightness are skipped as black. */
export const TINT_MIN_LIGHTNESS = 0.12;
/** Pixels lighter than this OKLab lightness are skipped as white. */
export const TINT_MAX_LIGHTNESS = 0.96;

/** Pixels with less OKLab chroma than this count as grey. */
export const TINT_COLORFUL_MIN_CHROMA = 0.04;
/** Art with a smaller share of colorful pixels is black, white or grey. */
export const TINT_COLORFUL_MIN_SHARE = 0.05;
/** A second main color of art is at least this far from the first around the color wheel, in degrees. */
export const TINT_SECOND_MIN_GAP_DEG = 60;
/** A second main color of art counts when it has at least this share of the colorfulness of the first. */
export const TINT_SECOND_MIN_SHARE = 0.4;
/** Bands the color wheel is cut into when looking for the main color of art. */
export const TINT_HUE_BINS = 24;

/** Mean OKLab chroma of a typical colorful cover, it maps to full tint strength. */
export const TINT_TYPICAL_CHROMA = 0.09;

/** How far the tint of colorful art may fade towards grey. Black, white and grey art goes all the way. */
export const TINT_MIN_STRENGTH = 0.2;
/** How far a tint may go past the default colorfulness. */
export const TINT_MAX_STRENGTH = 1.2;

// Appearance

/**
 * Ground tones: the hue of the ground and how colorful it is, where 1 is the
 * violet ink Vaultime started with.
 */
export const GROUNDS = {
  vault: { hue: 300, chroma: 1 },
  graphite: { hue: 260, chroma: 0.15 },
  midnight: { hue: 252, chroma: 1.25 },
  moss: { hue: 160, chroma: 0.85 },
  umber: { hue: 55, chroma: 0.9 },
  garnet: { hue: 10, chroma: 1 },
} as const;

/**
 * Lightness and chroma of each ground token in dark and light mode, at a
 * ground chroma of 1. Dark mode with the vault ground is the palette of
 * Vaultime 0.2. Light grounds are tinted paper, colorful enough that each
 * ground tone shows, with surfaces a little lighter than the page.
 */
export const GROUND_LEVELS = {
  ink: { dark: { lightness: 0.158, chroma: 0.019 }, light: { lightness: 0.955, chroma: 0.02 } },
  bar: { dark: { lightness: 0.175, chroma: 0.023 }, light: { lightness: 0.935, chroma: 0.026 } },
  surface: { dark: { lightness: 0.193, chroma: 0.025 }, light: { lightness: 0.985, chroma: 0.009 } },
  raised: { dark: { lightness: 0.209, chroma: 0.03 }, light: { lightness: 0.915, chroma: 0.03 } },
  rule: { dark: { lightness: 0.245, chroma: 0.04 }, light: { lightness: 0.885, chroma: 0.032 } },
  hairline: { dark: { lightness: 0.272, chroma: 0.041 }, light: { lightness: 0.855, chroma: 0.036 } },
  "hairline-strong": { dark: { lightness: 0.335, chroma: 0.056 }, light: { lightness: 0.78, chroma: 0.045 } },
  idle: { dark: { lightness: 0.395, chroma: 0.054 }, light: { lightness: 0.75, chroma: 0.05 } },
  faint: { dark: { lightness: 0.72, chroma: 0.043 }, light: { lightness: 0.48, chroma: 0.042 } },
  soft: { dark: { lightness: 0.873, chroma: 0.03 }, light: { lightness: 0.33, chroma: 0.035 } },
  text: { dark: { lightness: 0.94, chroma: 0.02 }, light: { lightness: 0.2, chroma: 0.03 } },
};

/**
 * Accent colors to pick from, violet first. Each keeps its hue, and its
 * chroma is cut to what fits sRGB at the accent lightness of the mode.
 */
export const ACCENT_SWATCHES = {
  violet: { hue: BRAND_HUE_DEG, chroma: 0.187 },
  blue: { hue: 255, chroma: 0.16 },
  teal: { hue: 195, chroma: 0.13 },
  green: { hue: 150, chroma: 0.16 },
  orange: { hue: 50, chroma: 0.17 },
  rose: { hue: 5, chroma: 0.18 },
} as const;

/**
 * Lightness of the accent, of the accent under the pointer and of text on
 * the accent, per mode. Every accent takes these, so any color reads on the
 * ground. Grey accents take their own lightness.
 */
export const ACCENT_LEVELS = {
  dark: { lightness: 0.678, hover: 0.748, ink: 0.183, inkChroma: 0.04, grey: 0.86, greyHover: 0.93 },
  light: { lightness: 0.48, hover: 0.41, ink: 0.985, inkChroma: 0.01, grey: 0.3, greyHover: 0.22 },
};

/** Chroma of the accent under the pointer, relative to the accent. */
export const ACCENT_HOVER_CHROMA_SCALE = 0.77;
/** A picked color with less chroma than this is a grey accent. */
export const ACCENT_GREY_MAX_CHROMA = 0.03;
/** Colorful accents take at least this chroma, so they still read as a color. */
export const ACCENT_MIN_CHROMA = 0.08;

/** Warning, recovered and error colors per mode. */
export const SIGNAL_LEVELS = {
  amber: { dark: { lightness: 0.808, chroma: 0.127, hue: 75 }, light: { lightness: 0.5, chroma: 0.115, hue: 62 } },
  sky: { dark: { lightness: 0.755, chroma: 0.126, hue: 260 }, light: { lightness: 0.49, chroma: 0.15, hue: 258 } },
  destructive: {
    dark: { lightness: 0.737, chroma: 0.162, hue: 17 },
    light: { lightness: 0.52, chroma: 0.185, hue: 22 },
  },
};

/** Lightness of shadows under covers and panels, per mode. */
export const SCRIM_LIGHTNESS = { dark: 0, light: 0.32 };
/** Lightness of the faint washes on hover and around covers, per mode. */
export const GLINT_LIGHTNESS = { dark: 1, light: 0.18 };

/** Share of the ground laid over the background picture, in percent. */
export const BACKGROUND_DIM_PERCENT = { min: 40, max: 95, default: 78 };
/** Blur of the background picture, in pixels. */
export const BACKGROUND_BLUR_PX = { min: 0, max: 48, default: 16 };
/** Opacity of tinted headers in front of a background picture, in percent. */
export const BACKGROUND_HEADER_OPACITY_PERCENT = 86;
/** Wait after the last move of a slider or the color picker before the value is saved. */
export const APPEARANCE_SAVE_DELAY_MS = 400;
/** The background picture reaches this many blur radii past each edge, so the blur does not fade there. */
export const BACKGROUND_BLUR_BLEED = 2;
/** Hues around the wheel on the swatch that opens the color picker. */
export const ACCENT_WHEEL_STEPS = 6;
/** The look of a new install, Vaultime as it started. */
export const DEFAULT_LOOK = { mode: "dark", ground: "vault", accent: "violet" } as const;
/**
 * Least time between two updates of the icons and the title bar, while a color
 * is dragged in the picker. A single change goes out at once.
 */
export const WINDOW_LOOK_INTERVAL_MS = 120;
/** Key of the copy of the look in local storage, read before the first paint. public/theme-boot.js repeats it. */
export const APPEARANCE_CACHE_KEY = "vaultime.appearance";
