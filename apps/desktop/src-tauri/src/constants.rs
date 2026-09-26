// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tuning values and limits of the desktop core.

use std::time::Duration;

// Tracking

/// How often the tracker polls running processes. The frontend polls at the
/// same rate, see ACTIVE_POLL_MS in lib/constants.ts.
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Grace period so a quick alt-tab does not count as idle.
pub const FOREGROUND_GRACE: Duration = Duration::from_secs(15);
/// Idle threshold when the `idle_threshold_seconds` setting is missing or invalid.
/// Same as DEFAULT_IDLE_THRESHOLD_SECONDS in lib/constants.ts.
pub const DEFAULT_IDLE_THRESHOLD_SECS: u64 = 300;
/// Lowest idle threshold the setting can choose.
pub const MIN_IDLE_THRESHOLD_SECS: u64 = 5;
/// CPU usage in percent at or above which a game process counts as active.
pub const PROCESS_ACTIVITY_CPU_THRESHOLD: f32 = 0.5;
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

// Logging

/// Size at which the log file is rotated.
pub const LOG_MAX_FILE_BYTES: u128 = 2_000_000;
/// Rotated log files kept on disk.
pub const LOG_FILES_KEPT: usize = 3;

// Discovery

/// Folder depth searched for game executables in a folder scan.
pub const DISCOVERY_SCAN_DEPTH: usize = 4;
/// Folder depth searched for the main executable of a Steam game.
pub const STEAM_EXECUTABLE_SCAN_DEPTH: usize = 3;
/// Size head start for executables in the top folder of a Steam game.
pub const STEAM_TOP_LEVEL_BONUS_BYTES: u64 = 100_000_000;

// Artwork

/// Folder depth searched for artwork below the install and executable folders.
pub const ASSET_SCAN_DEPTH: usize = 3;
/// Most artwork candidates cached per scan.
pub const MAX_SCANNED_ASSETS: usize = 10;
/// Most assets per game returned with a preview.
pub const MAX_LIBRARY_PREVIEWS: usize = 20;
/// Score of artwork the user picked, above anything a scan can reach.
pub const ARTWORK_SCORE_USER_PICKED: i32 = 10_000;
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
/// Width of a cached cover. Covers are cropped to fill.
pub const COVER_WIDTH_PX: u32 = 720;
/// Height of a cached cover.
pub const COVER_HEIGHT_PX: u32 = 960;

// Backups

/// Format version written to backup manifests. Other versions are rejected.
pub const BACKUP_VERSION: u32 = 1;
/// Entries shown in the backup history.
pub const BACKUP_HISTORY_LIMIT: usize = 8;
/// Unix permissions of the files in a cloud backup archive.
pub const ARCHIVE_FILE_MODE: u32 = 0o644;
/// Read buffer size for hashing cloud backup archives.
pub const HASH_BUFFER_BYTES: usize = 64 * 1024;
/// Plaintext size of one encrypted cloud backup chunk.
pub const ENCRYPTION_CHUNK_BYTES: usize = 256 * 1024;
/// Cloud backup key length, as `ChaCha20Poly1305` requires.
pub const BACKUP_KEY_BYTES: usize = 32;
/// Nonce length, as `ChaCha20Poly1305` requires.
pub const NONCE_BYTES: usize = 12;
/// Random nonce prefix per archive. The rest of the nonce is the chunk index.
pub const NONCE_PREFIX_BYTES: usize = 4;
