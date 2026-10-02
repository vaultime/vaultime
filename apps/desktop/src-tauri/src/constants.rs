// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tuning values and limits of the desktop core.

use std::time::Duration;

// Tracking

/// How often the tracker polls running processes. The frontend polls at the
/// same rate, see `ACTIVE_POLL_MS` in `lib/constants.ts`.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Longest note on a session, in characters. Same as `SESSION_NOTE_MAX_CHARS`
/// in `lib/constants.ts`.
pub const SESSION_NOTE_MAX_CHARS: usize = 280;
/// Longest session a player can add by hand. Same as
/// `MANUAL_SESSION_MAX_HOURS` in `lib/constants.ts`.
pub const MANUAL_SESSION_MAX: Duration = Duration::from_hours(24);
/// Grace period so a quick alt-tab does not count as idle.
pub const FOREGROUND_GRACE: Duration = Duration::from_secs(15);
/// How long the tracker reuses install folders with their links resolved
/// before it asks the file system again.
pub const INSTALL_FOLDER_REFRESH: Duration = Duration::from_mins(10);
/// Idle threshold when the `idle_threshold_seconds` setting is missing or invalid.
/// Same as `DEFAULT_IDLE_THRESHOLD_SECS` in `lib/constants.ts`.
pub const DEFAULT_IDLE_THRESHOLD_SECS: u64 = 300;
/// Lowest idle threshold the setting can choose. Same as
/// `MIN_IDLE_THRESHOLD_SECS` in `lib/constants.ts`.
pub const MIN_IDLE_THRESHOLD_SECS: u64 = 5;
/// CPU usage in percent at or above which a game process counts as active.
pub const PROCESS_ACTIVITY_CPU_PERCENT: f32 = 0.5;
/// A wall clock step back by more than this within one tick flags the session.
pub const CLOCK_BACKWARDS_TOLERANCE_MS: i64 = 1_000;
/// Allowed difference between wall and monotonic time within one tick.
pub const CLOCK_STEP_TOLERANCE_MS: i64 = 20_000;
/// Allowed drift between wall and monotonic time over a whole session.
pub const CLOCK_TOTAL_DRIFT_TOLERANCE_MS: i64 = 45_000;
/// A longer pause between ticks means the machine slept or the tracker stalled.
/// That time is not counted. On Windows the monotonic clock keeps running
/// during sleep and on Linux it stops, so both clocks are checked.
pub const MAX_TICK_GAP_MS: i64 = 60_000;
/// How far the clock that counts through sleep may run ahead of the
/// monotonic clock in one tick before the tick counts as asleep, in
/// milliseconds. Covers timer resolution, not real sleep.
pub const SUSPEND_DETECT_MS: i64 = 2_000;
/// How often connected controllers are read on Windows. `XInput` only reports
/// the current state, so this has to be short enough to catch a quick press.
pub const CONTROLLER_SAMPLE_INTERVAL: Duration = Duration::from_millis(250);
/// A stick or trigger counts as controller input once it moves more than its
/// range divided by this. Smaller moves are noise.
pub const CONTROLLER_AXIS_MOVE_DIVISOR: u32 = 8;
/// Input events read from a controller in one go on Linux.
pub const CONTROLLER_EVENTS_PER_READ: usize = 64;

// Data folder

/// The database in Vaultime's data folder, under the same name in backups.
pub const DATABASE_FILE: &str = "vaultime.db";
/// Folder of cached artwork in Vaultime's data folder, under the same name
/// in backups.
pub const ASSET_CACHE_DIR: &str = "asset-cache";

// Device

/// File next to the database that holds this PC's device id.
pub const DEVICE_ID_FILE: &str = "device-id";

// Window

/// Setting with the window size picked in Settings, the name of a preset or
/// [`FREE_WINDOW`]. Belongs to this PC, a restore keeps it.
pub const WINDOW_SIZE_SETTING: &str = "window_size";
/// Fixed window sizes as name, width and height of the page in logical
/// pixels, smallest first. All keep the 16:10 shape. The window in
/// `tauri.conf.json` is created at the smallest, because GTK never shrinks a
/// window that cannot be resized below the size it was created with. The
/// names are the `WindowSizeChoice` values in `lib/types.ts`.
pub const WINDOW_PRESETS: [(&str, u32, u32); 4] = [
    ("compact", 1024, 640),
    ("standard", 1280, 800),
    ("large", 1536, 960),
    ("extra_large", 1920, 1200),
];
/// The preset of a new install and the size a free window opens at.
pub const DEFAULT_WINDOW_PRESET: &str = "standard";
/// Window size choice that lets the player resize and maximize the window.
pub const FREE_WINDOW: &str = "free";
/// Narrowest page of a free window, in logical pixels. Same as `minWidth` in
/// `tauri.conf.json`.
pub const WINDOW_MIN_WIDTH_PX: f64 = 900.0;
/// Lowest page of a free window, in logical pixels. Same as `minHeight` in
/// `tauri.conf.json`.
pub const WINDOW_MIN_HEIGHT_PX: f64 = 600.0;
/// Height of the title bar in logical pixels, for when the system reports a
/// smaller frame, as before the window was first shown.
pub const WINDOW_TITLE_BAR_MIN_PX: f64 = 32.0;
/// The same on Linux, where GNOME's header bar takes about 37 to 46 pixels.
pub const WINDOW_TITLE_BAR_MIN_LINUX_PX: f64 = 48.0;
/// Room left for a top bar or dock on Wayland, where the work area is the
/// whole monitor, in logical pixels.
pub const WINDOW_WAYLAND_PANEL_PX: f64 = 64.0;

// Logging

/// Size at which the log file is rotated.
pub const LOG_MAX_FILE_BYTES: u128 = 2_000_000;
/// Rotated log files kept on disk.
pub const LOG_FILES_KEPT: usize = 3;

// Discovery

/// Folder depth searched for game executables in a folder scan.
pub const DISCOVERY_SCAN_DEPTH: usize = 4;
/// Registry levels searched below a launcher key for the games it installed.
pub const REGISTRY_SEARCH_DEPTH: usize = 6;
/// Folder depth searched for the main executable of a launcher game.
pub const EXECUTABLE_SCAN_DEPTH: usize = 5;
/// Size head start for executables in the top folder of a launcher game.
pub const TOP_LEVEL_BONUS_BYTES: u64 = 100_000_000;
/// Head start for an executable named like the game, bigger than any top
/// folder bonus but smaller than a whole game binary.
pub const TITLE_MATCH_BONUS_BYTES: u64 = 500_000_000;
/// Head start for an Unreal Engine `-Shipping` build, which is always the
/// game itself, so it wins against everything else.
pub const SHIPPING_BONUS_BYTES: u64 = 1_000_000_000_000;
/// Shortest title word that counts when matching executable names.
pub const TITLE_WORD_MIN_CHARS: usize = 3;
/// Launcher source of Steam games and of playtime read from Steam. Same as
/// `STEAM_LAUNCHER` in `lib/constants.ts`.
pub const STEAM_SOURCE: &str = "steam";

// Artwork

/// Folder depth searched for artwork below the install and executable folders.
pub const ASSET_SCAN_DEPTH: usize = 3;
/// Most artwork candidates cached per scan.
pub const MAX_SCANNED_ASSETS: usize = 10;
/// Most assets per game returned with a preview.
pub const MAX_LIBRARY_PREVIEWS: usize = 20;
/// Largest image file read as artwork, in bytes. Covers are far smaller, so
/// a huge file in a game folder cannot fill the memory.
pub const MAX_ARTWORK_SOURCE_BYTES: u64 = 64 * 1024 * 1024;
/// Widest and tallest image decoded as artwork, in pixels. A small file can
/// claim a huge image, and decoding it would fill the memory.
pub const ARTWORK_MAX_SIDE_PX: u32 = 16_384;
/// Memory the decoder may take for one image, in bytes.
pub const ARTWORK_DECODE_MAX_BYTES: u64 = 256 * 1024 * 1024;
/// Source of an image the player added or framed. Same as
/// `PLAYER_ARTWORK_SOURCE` in `lib/constants.ts`.
pub const PLAYER_ARTWORK_SOURCE: &str = "user_picked";
/// Score of the cover from Steam's own library cache, above anything a
/// folder scan finds.
pub const ARTWORK_SCORE_STEAM_COVER: i32 = 1_000;
/// Score for "cover" or "capsule" in the file name.
pub const ARTWORK_SCORE_COVER: i32 = 120;
/// Score for "poster" or "banner" in the file name.
pub const ARTWORK_SCORE_POSTER: i32 = 100;
/// Score for "hero" or "art" in the file name.
pub const ARTWORK_SCORE_HERO: i32 = 80;
/// Score for "logo" or "icon" in the file name.
pub const ARTWORK_SCORE_LOGO: i32 = 60;
/// Score for "screenshot" or "screen" in the file name.
pub const ARTWORK_SCORE_SCREENSHOT: i32 = 20;
/// Penalty for "screenshot" anywhere in the path.
pub const ARTWORK_PENALTY_SCREENSHOT_PATH: i32 = 20;
/// Width of a cached banner. Banners are cropped to fill.
pub const BANNER_WIDTH_PX: u32 = 1280;
/// Height of a cached banner.
pub const BANNER_HEIGHT_PX: u32 = 720;
/// Longest edge of a cached icon.
pub const ICON_MAX_SIZE_PX: u32 = 512;
/// Largest width of a cached screenshot. Screenshots keep their aspect ratio.
pub const SCREENSHOT_MAX_WIDTH_PX: u32 = 1280;
/// Largest height of a cached screenshot.
pub const SCREENSHOT_MAX_HEIGHT_PX: u32 = 720;
/// Setting that records which artwork backfill already ran.
pub const ARTWORK_BACKFILL_SETTING: &str = "artwork_backfill";
/// Raise it to scan Steam games for artwork once more after an artwork change.
pub const ARTWORK_BACKFILL_VERSION: &str = "1";
/// Event that tells the frontend to reload the library. Same as
/// `LIBRARY_CHANGED_EVENT` in `lib/constants.ts`.
pub const LIBRARY_CHANGED_EVENT: &str = "library-changed";
/// JPEG quality of cached covers, banners and screenshots. Icons stay PNG
/// for their transparency.
pub const CACHED_JPEG_QUALITY: u8 = 85;
/// Width of a cached cover. Covers are cropped to fill. Same as
/// `COVER_WIDTH_PX` in `lib/constants.ts`, whose `COVER_ASPECT` is this width
/// over `COVER_HEIGHT_PX`.
pub const COVER_WIDTH_PX: u32 = 720;
/// Height of a cached cover.
pub const COVER_HEIGHT_PX: u32 = 960;

// Artwork crop

/// Longest edge of the image the crop dialog shows.
pub const CROP_PREVIEW_MAX_PX: u32 = 1200;
/// Longest edge an SVG is drawn at before it is cropped.
pub const SVG_RENDER_MAX_PX: u32 = 2048;
/// An SVG that needs a smaller drawing than this to stay within
/// `SVG_MAX_LAYER_BYTES` is refused, in pixels on the longest edge.
pub const SVG_RENDER_MIN_PX: u32 = 256;
/// Memory the layers of nested see-through, clipped, masked or filtered
/// groups of an SVG may take at once, in bytes. resvg gives each such group a
/// layer of its own, up to five times the drawing each way.
pub const SVG_MAX_LAYER_BYTES: u64 = 256 * 1024 * 1024;
/// Deepest nesting of such groups an SVG may have.
pub const SVG_MAX_LAYER_DEPTH: usize = 8;
/// Most shapes and groups an SVG may have, so drawing it stays quick.
pub const SVG_MAX_NODES: usize = 20_000;
/// Most filters an SVG may use, as each one is drawn into layers of its own.
pub const SVG_MAX_FILTERS: usize = 32;
/// A crop frame may be this many times the frame that holds the whole image.
/// The dialog stops at `CROP_MIN_ZOOM_OF_FIT` in `lib/constants.ts`, well
/// before it, so rounding never refuses a crop the dialog made.
pub const CROP_MAX_FRAME_OF_FIT: f64 = 4.0;
/// Width of the backdrop behind a cropped cover before it is scaled up. The
/// height follows the cover.
pub const CROP_BACKDROP_WIDTH_PX: u32 = 90;
/// Blur of the backdrop, in its own small pixels.
pub const CROP_BACKDROP_BLUR_SIGMA_PX: f32 = 3.0;
/// Share of its brightness the blurred image keeps in the backdrop, so the
/// cover in front of it stands out.
pub const CROP_BACKDROP_BRIGHTNESS: f32 = 0.45;
/// Images are scaled down to this many pixels square to tell whether they
/// are see-through and to find the color of the plain behind them.
pub const CROP_BACKDROP_SAMPLE_PX: u32 = 32;
/// Art that covers less than this share of its area is see-through, like a
/// logo, and gets a plain backdrop instead of a blurred copy of itself.
pub const CROP_BACKDROP_OPAQUE_COVERAGE: f64 = 0.99;
/// Art at least this light, as sRGB luma from 0 to 1, gets a dark plain
/// behind it, darker art a light one.
pub const CROP_BACKDROP_LIGHT_ART_LUMA: f64 = 0.5;
/// Luma of the dark plain behind light art.
pub const CROP_BACKDROP_DARK_PLAIN_LUMA: f64 = 0.12;
/// Luma of the light plain behind dark art.
pub const CROP_BACKDROP_LIGHT_PLAIN_LUMA: f64 = 0.82;
/// Plain behind art without a single opaque pixel. Same as the default
/// `--surface` in `index.css`.
pub const CROP_BACKDROP_FALLBACK_RGB: [u8; 3] = [22, 18, 30];

// Backups

/// Format version written to backup manifests. Other versions are rejected.
pub const BACKUP_VERSION: u32 = 1;
/// Entries shown in the backup history.
pub const BACKUP_HISTORY_LIMIT: usize = 8;
/// Folder in Vaultime's data folder for automatic backups, unless the user
/// picks another one.
pub const AUTO_BACKUP_DIR: &str = "backups";
/// Setting with the idle threshold in seconds. Same as
/// `SETTING_KEYS.idleThreshold` in `lib/constants.ts`.
pub const IDLE_THRESHOLD_SETTING: &str = "idle_threshold_seconds";
/// Setting that counts time in the background as active with `true`. Same as
/// `SETTING_KEYS.backgroundActive` in `lib/constants.ts`.
pub const BACKGROUND_ACTIVE_SETTING: &str = "treat_background_as_active";
/// Setting that decides whether closing the window keeps Vaultime in the
/// tray. Same as `SETTING_KEYS.closeToTray` in `lib/constants.ts`.
pub const CLOSE_TO_TRAY_SETTING: &str = "close_to_tray";
/// Setting that turns daily cloud backups off with `false`. Same as
/// `SETTING_KEYS.cloudAutoBackup` in `lib/constants.ts`.
pub const CLOUD_AUTO_BACKUP_SETTING: &str = "cloud_auto_backup_enabled";
/// Setting with how the rail sorts the games. Same as `SETTING_KEYS.railSort`
/// in `lib/constants.ts`.
pub const RAIL_SORT_SETTING: &str = "rail_sort";
/// Setting with the folder the user picked for automatic backups. Same as
/// `SETTING_KEYS.autoBackupFolder` in `lib/constants.ts`.
pub const AUTO_BACKUP_FOLDER_SETTING: &str = "auto_backup_folder";
/// Setting that turns automatic backups off with `false`. Same as
/// `SETTING_KEYS.autoBackup` in `lib/constants.ts`.
pub const AUTO_BACKUP_ENABLED_SETTING: &str = "auto_backup_enabled";
/// The settings the page may change. The others, such as the artwork scan
/// version, belong to the core. Same as `SETTING_KEYS` in `lib/constants.ts`.
pub const PAGE_SETTINGS: &[&str] = &[
    IDLE_THRESHOLD_SETTING,
    BACKGROUND_ACTIVE_SETTING,
    CLOSE_TO_TRAY_SETTING,
    AUTO_BACKUP_ENABLED_SETTING,
    AUTO_BACKUP_FOLDER_SETTING,
    CLOUD_AUTO_BACKUP_SETTING,
    RAIL_SORT_SETTING,
    APPEARANCE_MODE_SETTING,
    APPEARANCE_GROUND_SETTING,
    APPEARANCE_ACCENT_SETTING,
    BACKGROUND_DIM_SETTING,
    BACKGROUND_BLUR_SETTING,
];
/// Folder name prefix of automatic backups, so pruning never touches others.
pub const AUTO_BACKUP_PREFIX: &str = "vaultime-auto";
/// Automatic backups kept, older ones are deleted. Same as `AUTO_BACKUP_KEEP`
/// in `lib/constants.ts`.
pub const AUTO_BACKUP_KEEP: usize = 7;
/// Age of the newest automatic backup at which the daily one is due.
pub const AUTO_BACKUP_INTERVAL: Duration = Duration::from_hours(24);
/// Quitting makes a backup only when the last one is at least this old.
pub const AUTO_BACKUP_ON_QUIT_MIN_AGE: Duration = Duration::from_hours(1);
/// How often the running app checks whether the daily backup is due.
pub const AUTO_BACKUP_CHECK_INTERVAL: Duration = Duration::from_hours(1);
/// Unix permissions of the files in a cloud backup archive.
pub const ARCHIVE_FILE_MODE: u32 = 0o644;
/// Read buffer size for hashing backup files.
pub const HASH_BUFFER_BYTES: usize = 64 * 1024;
/// Plaintext size of one encrypted cloud backup chunk.
pub const ENCRYPTION_CHUNK_BYTES: usize = 256 * 1024;
/// Longest time one backup or artwork upload or download may take. Small
/// requests keep the HTTP client's default limit.
pub const CLOUD_TRANSFER_TIMEOUT: Duration = Duration::from_mins(30);
/// Cloud backup key length, as `XChaCha20Poly1305` requires.
pub const BACKUP_KEY_BYTES: usize = 32;
/// How often the tray menu updates the running game and today's play.
pub const TRAY_STATUS_INTERVAL: Duration = Duration::from_secs(20);
/// Less play today than this reads "Nothing played today" in the tray menu.
pub const TRAY_TODAY_MIN_PLAYED_MS: i64 = 60_000;
/// Name of the JSON session export, so tools can tell it apart.
pub const EXPORT_FORMAT_NAME: &str = "vaultime-sessions";
/// Version of the JSON session export, raised when its fields change.
pub const EXPORT_FORMAT_VERSION: u32 = 1;
/// Bytes of the key check stored with each cloud backup, enough to tell two
/// keys apart. Part of the backup format, changing it breaks the check.
pub const KEY_CHECK_BYTES: usize = 16;
/// Address of the cloud server. Release builds use only this one, whatever the
/// page asks. Same as `VAULTIME_URL` in `lib/constants.ts`.
pub const CLOUD_API_BASE_URL: &str = "https://vaultime.codfishcloud.de";
/// Most artwork ids asked about in one request. Same as
/// `MAX_BLOB_IDS_PER_REQUEST` in the API's `constants.rs`.
pub const MAX_ARTWORK_IDS_PER_REQUEST: usize = 10_000;

// Appearance

/// Setting with dark, light or the system's mode. Same as
/// `SETTING_KEYS.appearanceMode` in `lib/constants.ts`.
pub const APPEARANCE_MODE_SETTING: &str = "appearance_mode";
/// Setting with the ground tone. Same as `SETTING_KEYS.appearanceGround` in
/// `lib/constants.ts`.
pub const APPEARANCE_GROUND_SETTING: &str = "appearance_ground";
/// Setting with the accent color. Same as `SETTING_KEYS.appearanceAccent` in
/// `lib/constants.ts`.
pub const APPEARANCE_ACCENT_SETTING: &str = "appearance_accent";
/// Setting with how far the background picture is dimmed. Same as
/// `SETTING_KEYS.backgroundDim` in `lib/constants.ts`.
pub const BACKGROUND_DIM_SETTING: &str = "background_dim";
/// Setting with how far the background picture is blurred. Same as
/// `SETTING_KEYS.backgroundBlur` in `lib/constants.ts`.
pub const BACKGROUND_BLUR_SETTING: &str = "background_blur";
/// Folder in Vaultime's data folder for the background picture. Backups
/// leave it out.
pub const APPEARANCE_DIR: &str = "appearance";
/// File name of the stored background picture.
pub const BACKGROUND_FILE: &str = "background.jpg";
/// Longest side of the stored background picture, enough for a large window
/// on a 4K screen.
pub const BACKGROUND_MAX_SIDE_PX: u32 = 2560;
/// JPEG quality of the stored background picture.
pub const BACKGROUND_JPEG_QUALITY: u8 = 86;
/// Largest picture file read as a background, in bytes.
pub const BACKGROUND_SOURCE_MAX_BYTES: u64 = 64 * 1024 * 1024;
/// Setting with the colors of the window around the page, as JSON. The core
/// keeps it, so the next start draws the icons and the title bar before the
/// page loads. Belongs to this PC, a restore keeps it.
pub const WINDOW_LOOK_SETTING: &str = "window_look";
/// Size the icon of the tray, the taskbar and the title bar is drawn at, the
/// size of `icons/signed-in.png`.
pub const WINDOW_ICON_PX: u32 = 256;
