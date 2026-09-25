# Privacy Policy

**Effective date:** April 22, 2026

Vaultime is a local-first desktop application for tracking PC game playtime. This policy explains what data we collect, how we use it, and how we protect it.

## 1. Local-Only Data

By default, all data stays on your device:

- **Game library** — titles, executable paths, and install folders you register.
- **Session history** — playtime sessions with start/end times, active/idle/runtime durations, and integrity metadata.
- **Artwork** — cached images scanned from local game folders or manually imported.
- **Settings** — your tracking preferences (idle threshold, background-as-active toggle).
- **Device identity** — a hostname-based device identifier stored in the local database.

This data is stored in a SQLite database in your operating system's application data directory. We never access, collect, or transmit local-only data.

## 2. Cloud Features (Optional)

The current product direction is local-first. If you only use local tracking and
local backup/export, we do not receive your data.

If an invite-only self-hosted cloud backup service is introduced later, this
policy will be updated before rollout. That future service is expected to use:

- **Account data** such as email address and account status
- **Device registration** metadata such as device identifier, platform, and app version
- **Session event metadata** when server-backed verification is enabled
- **Encrypted backup archives** containing your exported local data

## 3. Data We Do Not Collect

- No analytics or telemetry.
- No crash reports.
- No usage tracking or behavioral profiling.
- No advertising identifiers.
- No data from other applications besides process name matching for tracking.

## 4. Third-Party Services

The current local-first build does not require any third-party cloud provider
for core tracking or local backups.

## 5. Data Retention

- **Local data** is retained until you delete the app or its data directory.
- **Cloud data** retention rules will be documented before any invite-only cloud
  backup service is launched.

## 6. Data Deletion

- Delete local data by removing the Vaultime application data directory.
- If invite-only cloud backup is introduced later, deletion instructions will be
  documented here before launch.

## 7. Security

- Local backup exports are created on your device.
- Session event integrity is protected by hash chains and monotonic time validation.
- Any future cloud communication will use HTTPS and documented account access controls.

## 8. Children

Vaultime is not directed at children under 13. We do not knowingly collect data from children.

## 9. Changes

We may update this policy as the product evolves. Material changes will be noted in the changelog.

## 10. Contact

For privacy questions or data deletion requests, open an issue at [github.com/schwimmbeck/vaultime](https://github.com/schwimmbeck/vaultime) or email the project maintainer.
