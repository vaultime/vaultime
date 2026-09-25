# Changelog

All notable changes to Vaultime will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
