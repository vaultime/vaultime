// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Tuning values and limits of the cloud API.

// Units

/// Bytes in one mebibyte.
pub const BYTES_PER_MIB: i64 = 1024 * 1024;
/// Bytes in one gibibyte.
pub const BYTES_PER_GIB: i64 = 1024 * BYTES_PER_MIB;
/// Seconds in one minute.
pub const SECS_PER_MINUTE: i64 = 60;
/// Seconds in one hour.
pub const SECS_PER_HOUR: u64 = 60 * 60;

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
/// Shortest accepted password, in characters. Same as `MIN_PASSWORD_CHARS` in
/// `deploy/vps/bootstrap-admin-account.py`.
pub const MIN_PASSWORD_CHARS: usize = 10;

// Abuse limits
//
// Kept in memory only. See `limits.rs`.

/// Window the rate limits count requests in, in seconds.
pub const RATE_WINDOW_SECS: u64 = 10 * 60;
/// Sign-up, sign-in, refresh and password requests one client address may
/// make per window. The app refreshes about four times an hour.
pub const AUTH_REQUESTS_PER_WINDOW_PER_CLIENT: u32 = 60;
/// Failed sign-ins per email address and window, from anywhere.
pub const LOGIN_FAILURES_PER_WINDOW_PER_EMAIL: u32 = 30;
/// Failed sign-ins per email address and window from one client address.
pub const LOGIN_FAILURES_PER_WINDOW_PER_EMAIL_AND_CLIENT: u32 = 10;
/// Beta applications one client address may send per hour.
pub const BETA_APPLICATIONS_PER_HOUR_PER_CLIENT: u32 = 3;
/// Keys a rate limiter tracks at most, so a flood of addresses cannot fill the memory.
pub const RATE_LIMIT_MAX_KEYS: usize = 100_000;
/// Password and invite hashes computed at the same time.
pub const MAX_CONCURRENT_PASSWORD_HASHES: usize = 4;
/// Longest wait for a free hashing slot before a sign-in is turned away, in seconds.
pub const HASH_QUEUE_WAIT_SECS: u64 = 10;
/// Longest wait for another upload of the same account to finish, in seconds.
pub const UPLOAD_LOCK_WAIT_SECS: u64 = 60;
/// Longest pause between two parts of an upload before it counts as stalled, in seconds.
pub const UPLOAD_IDLE_SECS: u64 = 120;
/// Devices one account may register.
pub const MAX_DEVICES_PER_ACCOUNT: i64 = 50;
/// Longest device id, name, platform or app version, in characters.
pub const MAX_DEVICE_FIELD_CHARS: usize = 200;
/// Longest device public key, in characters.
pub const MAX_DEVICE_PUBLIC_KEY_CHARS: usize = 4096;
/// How often the server removes stale uploads, unused artwork, expired
/// sessions and old beta applications, in seconds.
pub const MAINTENANCE_INTERVAL_SECS: u64 = 60 * 60;

// Invites

/// Redemptions of an admin invite when the request sets none.
pub const DEFAULT_INVITE_MAX_REDEMPTIONS: i32 = 1;

// Invite codes. `deploy/vps/generate-cloud-invite.py` and
// `bootstrap-admin-account.py` use the same values. Changing the scrypt
// settings breaks every stored invite hash.

/// Prefix of invite codes when the request sets none. Also in
/// `lib/cloud-api.ts` of the desktop app and in `scripts/cloud-e2e.sh`.
pub const INVITE_PREFIX: &str = "VTLINV";

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
pub const INVITE_BODY_CHARS: usize = 24;
/// Leading body characters stored in plain text to look an invite up.
pub const INVITE_LOOKUP_KEY_CHARS: usize = 12;
/// Characters per dash separated group in an invite code.
pub const INVITE_CODE_GROUP_CHARS: usize = 4;

// Backup limits
//
// Defaults for the `VAULTIME_*` backup settings. They only exist to stop abuse.

/// Largest accepted backup upload in bytes. Also the largest artwork blob.
pub const DEFAULT_MAX_BACKUP_BYTES: i64 = 512 * BYTES_PER_MIB;
/// Storage one account may use for backups and artwork together, in bytes,
/// counted as stored: compressed and encrypted.
pub const DEFAULT_MAX_ACCOUNT_BYTES: i64 = BYTES_PER_GIB;
/// Unfinished uploads allowed per account at the same time.
pub const DEFAULT_MAX_PENDING_BACKUPS_PER_ACCOUNT: i64 = 1;
/// Complete backups kept per account before the oldest are rotated out.
pub const DEFAULT_MAX_COMPLETE_BACKUPS_PER_ACCOUNT: i64 = 30;
/// Shortest time between two backups of an account, in seconds.
pub const DEFAULT_MIN_BACKUP_INTERVAL_SECS: i64 = 15 * SECS_PER_MINUTE;
/// Age in seconds after which an unfinished upload is removed. Artwork that no
/// backup refers to is kept this long too, so a backup being assembled keeps
/// the artwork it just uploaded.
pub const DEFAULT_STALE_PENDING_BACKUP_SECS: i64 = 60 * SECS_PER_MINUTE;

// Beta applications

/// Longest email address an account or a beta application takes, the limit
/// of the address format. Same as the `maxlength` of the email field in
/// `docs/site/index.html`.
pub const EMAIL_MAX_CHARS: usize = 254;
/// Longest note on a beta application. Same as the `maxlength` of the note
/// field in `docs/site/index.html`.
pub const BETA_NOTE_MAX_CHARS: usize = 500;
/// Applications taken per hour from everyone together. Only stops floods.
pub const BETA_APPLICATIONS_PER_HOUR: i64 = 30;
/// Days after which an unanswered application is deleted.
pub const BETA_APPLICATION_RETENTION_DAYS: i32 = 90;

// Artwork blobs

/// Most blob ids a client may ask about or refer to in one request. Same as
/// `MAX_ARTWORK_IDS_PER_REQUEST` in the desktop core's `constants.rs`.
pub const MAX_BLOB_IDS_PER_REQUEST: usize = 10_000;
