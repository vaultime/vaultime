# Changelog

All notable changes to Vaultime will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Self-hosted cloud API.** Invite-only accounts, device registration and encrypted backup storage in `apps/api`, replacing Supabase.
- **Backup deletion and rotation.** Users can delete cloud backups, and a new upload replaces the oldest one at the account limit instead of failing.
- **Windows support.** Registry based Steam detection, launcher folders on every fixed drive, case-insensitive path matching, and CI checks on Windows.
- **Sleep handling.** Time while the machine sleeps is skipped and logged as a `tracking_gap` event instead of counted or flagged.
- **Log files.** Logs are written to the app log folder, which matters on Windows where release builds have no console.
- **Single instance.** Starting Vaultime twice focuses the running window instead of tracking every game twice.

### Changed

- **Dependencies.** Upgraded to the current stable versions: Tauri 2.12, React 19.3, React Router 8, TypeScript 6, ESLint 10, rusqlite 0.40, sysinfo 0.39, sqlx 0.9 and more. The Rust toolchain is pinned in `rust-toolchain.toml`.
- **Restores.** Older backups are migrated to the current schema before import, and backups from newer versions are rejected with a clear message.
- **Slow commands.** Backup, restore, discovery and artwork commands run off the main thread, so the window stays responsive.
- **CI and releases.** CI checks the frontend, the desktop core on Windows and Linux, and the API. Releases build Windows and Linux installers with signed updater manifests through `tauri-action`. macOS builds were dropped for now.

### Fixed

- **Secure storage.** The keychain crate was built without a native backend, so the cloud session and backup passphrase were lost on every restart. They now use Windows Credential Manager or the Secret Service.
- **Windows idle time.** Idle time was wrong after about 49.7 days of uptime.
- **Restored artwork.** Restored backups pointed cached artwork at files that did not exist.
- **Tracking after a failed restore.** A failed restore used to stop tracking until the app restarted.
- **Library polling.** The library no longer reloads all sessions and artwork every 5 seconds.
- **Deploy scripts.** Fixed paths to `deploy/vps` and removed the default admin password.
- **Legal pages.** The privacy policy and terms now describe the cloud backup that exists, including exactly what the server stores.

### Removed

- **Billing leftovers.** The Pro recolor of the sidebar branding, the unused event sync columns and code, and the unreachable `Verified` trust label.

## [0.1.0] - 2026-04-06

Initial release covering milestones 1 through 13.

### Added

- **Foundation** — Tauri 2 + React + TypeScript scaffold with sidebar navigation, Rust module structure, and IPC bridge.
- **Local database** — SQLite with WAL mode, forward-only migrations, and CRUD for games, sessions, settings, and devices.
- **Manual game registration** — add, edit, and delete games via native file-picker dialogs with title inference.
- **Tracking engine** — process detection, session start/end, runtime tracking with 5-second polling, and crash recovery for orphaned sessions.
- **Active playtime** — foreground window and idle detection on Linux (X11), Windows (Win32), and macOS (HID/osascript) with configurable thresholds.
- **Session history UI** — timeline views, recent-activity charts, totals, per-game detail pages, and cross-game rankings.
- **Image import** — local game folder scanning for artwork, thumbnail caching, manual asset override, and preferred-artwork selection.
- **Integrity system** — append-only session event log with hash chains, monotonic time comparison, and Local/Suspicious/Recovered trust badges.
- **Local backups** — self-contained folder exports with per-file checksums, restore with preview and overwrite warnings, and restart-after-restore flow.
- **Cloud foundation** — Supabase auth (sign-up, sign-in, token refresh, sign-out), device registration, and cloud configuration with compile-time env vars.
- **Cloud sync and backup** — event sync in batches to Supabase, cloud backup upload/restore via private Storage bucket, and Verified trust level from server acknowledgements.
- **Billing and subscription** — Stripe integration via Supabase Edge Functions (checkout, portal, webhook), Pro subscription gating on sync/backup, tier badge and renewal info in UI.
- **Auto-discovery** — Steam library detection (VDF/ACF parsing), common install folder scanning, bulk import dialog with source badges and duplicate detection.
- **Pro visual indicator** — sidebar logo and wordmark recolor for active Pro subscribers.
- **Cross-platform support** — native activity signals on Linux, Windows, and macOS with heuristic fallbacks.
