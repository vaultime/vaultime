// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Tuning values and limits of the cloud API.

// Units

/// Bytes in one mebibyte.
pub const BYTES_PER_MIB: i64 = 1024 * 1024;
/// Seconds in one minute.
pub const SECS_PER_MINUTE: i64 = 60;

// Database

/// Most open connections in the `PostgreSQL` pool.
pub const DB_MAX_CONNECTIONS: u32 = 10;

// Auth

/// Lifetime of an access token in minutes.
pub const ACCESS_TOKEN_TTL_MINUTES: i64 = 15;
/// Lifetime of a refresh token in days.
pub const REFRESH_TOKEN_TTL_DAYS: i64 = 30;
/// Random bytes in a refresh token, before base64url encoding.
pub const REFRESH_TOKEN_BYTES: usize = 32;
/// Shortest accepted password length.
pub const MIN_PASSWORD_LENGTH: usize = 10;

// Invites

/// Redemptions of an admin invite when the request sets none.
pub const DEFAULT_INVITE_MAX_REDEMPTIONS: i32 = 1;

// Invite codes. The VPS and Node invite generators use the same values. Changing the scrypt
// settings breaks every stored invite hash.

/// scrypt cost as log2 of N.
pub const INVITE_SCRYPT_LOG_N: u8 = 14;
/// scrypt block size.
pub const INVITE_SCRYPT_R: u32 = 8;
/// scrypt parallelism.
pub const INVITE_SCRYPT_P: u32 = 1;
/// Length of an invite hash in bytes, before hex encoding.
pub const INVITE_HASH_BYTES: usize = 64;
/// Random bytes in an invite salt, before hex encoding.
pub const INVITE_SALT_BYTES: usize = 16;
/// Random bytes drawn for an invite code body, before base64url encoding.
pub const INVITE_BODY_RANDOM_BYTES: usize = 18;
/// Characters in an invite code body, without the prefix and dashes.
pub const INVITE_BODY_LENGTH: usize = 24;
/// Leading body characters stored in plain text to look an invite up.
pub const INVITE_LOOKUP_KEY_LENGTH: usize = 12;
/// Characters per dash separated group in an invite code.
pub const INVITE_CODE_GROUP_LENGTH: usize = 4;

// Backup limits
//
// Defaults for the `VAULTIME_*` backup settings. They only exist to stop abuse.

/// Largest accepted backup upload in bytes.
pub const DEFAULT_MAX_BACKUP_BYTES: i64 = 512 * BYTES_PER_MIB;
/// Unfinished uploads allowed per account at the same time.
pub const DEFAULT_MAX_PENDING_BACKUPS_PER_ACCOUNT: i64 = 1;
/// Complete backups kept per account before the oldest are rotated out.
pub const DEFAULT_MAX_COMPLETE_BACKUPS_PER_ACCOUNT: i64 = 30;
/// Shortest time between two backups of an account, in seconds.
pub const DEFAULT_MIN_BACKUP_INTERVAL_SECS: i64 = 15 * SECS_PER_MINUTE;
/// Age in seconds after which an unfinished upload is removed.
pub const DEFAULT_STALE_PENDING_BACKUP_SECS: i64 = 60 * SECS_PER_MINUTE;
