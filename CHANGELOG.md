# Changelog

All notable changes to Vaultime will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **A new look.** The library opens on the game you play right now or last, in colors taken from its cover, with a shelf of recent games and every game below. Game pages show the last two weeks and every session as a sentence.
- **Journal.** Your play week by week, with a 24 hour strip for each day. It replaces the Sessions page.
- **Live bar.** The running game, its timer and the active and idle split stay at the bottom of every page.
- **Search with Ctrl+K.** Jump to any game, page or action.
- **Reasons for trust labels.** Flagged and recovered sessions say what happened, and time skipped during sleep is shown.
- **Invite-only cloud backup on our own server.** Free, with the same limits for every account. Backups are encrypted on your computer before upload.
- **Delete cloud backups** from the Cloud page. At the backup limit a new upload replaces your oldest backup instead of failing.
- **Windows support.** Steam games are found wherever Steam is installed, and games in Epic, GOG, Xbox, EA, Ubisoft and Battle.net folders on every drive.
- **Linux packages for more distributions.** Debian, Ubuntu, Fedora, Arch, openSUSE and others. Every release is installed and started on these before it is published.
- **Sleep handling.** Time while your computer sleeps is no longer counted.
- **Restart button** after restoring a backup.
- **Log files** in the app's log folder for troubleshooting.

### Changed

- **Restores** of backups from older versions now work, and backups from newer versions show a clear message instead of failing.
- **The window stays responsive** during backups, restores, discovery and artwork scans.
- **Starting Vaultime twice** brings the open window to the front instead of tracking every game twice.
- **Errors are shown** in dialogs and discovery instead of disappearing silently.
- **macOS builds** are not offered for now.
- **The idle time** is set in minutes, and a change shows up right away.
- **Dates** use English words with your region's date order and clock.

### Fixed

- **Cloud sign-in and backup passphrase** were forgotten on every restart.
- **Idle time on Windows** was wrong after about 49.7 days of uptime.
- **Artwork after a restore** pointed at missing files.
- **Tracking stopped** after a failed restore until Vaultime was restarted.
- **The library** reloaded all sessions and artwork every 5 seconds.
- **Charts** put play near midnight on the wrong day outside UTC.
- **Games with the same file name**, like two different `Game.exe`, no longer count as running when only one of them is. Libraries behind junctions or symlinks still match.

### Removed

- **Subscription leftovers.** The Pro branding recolor and the `Verified` trust label, which could never be earned.

## [0.1.0] - 2026-04-06

Initial release covering milestones 1 through 13.

### Added

- **Foundation.** Tauri 2 + React + TypeScript scaffold with sidebar navigation, Rust module structure, and IPC bridge.
- **Local database.** SQLite with WAL mode, forward-only migrations, and CRUD for games, sessions, settings, and devices.
- **Manual game registration.** Add, edit, and delete games via native file-picker dialogs with title inference.
- **Tracking engine.** Process detection, session start/end, runtime tracking with 5-second polling, and crash recovery for orphaned sessions.
- **Active playtime.** Foreground window and idle detection on Linux (X11), Windows (Win32), and macOS (HID/osascript) with configurable thresholds.
- **Session history UI.** Timeline views, recent-activity charts, totals, per-game detail pages, and cross-game rankings.
- **Image import.** Local game folder scanning for artwork, thumbnail caching, manual asset override, and preferred-artwork selection.
- **Integrity system.** Append-only session event log with hash chains, monotonic time comparison, and Local/Suspicious/Recovered trust badges.
- **Local backups.** Self-contained folder exports with per-file checksums, restore with preview and overwrite warnings, and restart-after-restore flow.
- **Cloud foundation.** Supabase auth (sign-up, sign-in, token refresh, sign-out), device registration, and cloud configuration with compile-time env vars.
- **Cloud sync and backup.** Event sync in batches to Supabase, cloud backup upload/restore via private Storage bucket, and Verified trust level from server acknowledgements.
- **Billing and subscription.** Stripe integration via Supabase Edge Functions (checkout, portal, webhook), Pro subscription gating on sync/backup, tier badge and renewal info in UI.
- **Auto-discovery.** Steam library detection (VDF/ACF parsing), common install folder scanning, bulk import dialog with source badges and duplicate detection.
- **Pro visual indicator.** Sidebar logo and wordmark recolor for active Pro subscribers.
- **Cross-platform support.** Native activity signals on Linux, Windows, and macOS with heuristic fallbacks.
