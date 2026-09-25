<p align="center">
  <img src="assets/timesafe.svg" alt="Vaultime icon" width="120" />
</p>

<p align="center">
  <img src="assets/vaultime.svg" alt="Vaultime" width="360" />
</p>

<p align="center">
  <strong>Universal game library and trustworthy playtime tracker.</strong><br />
  Local-first. Windows and Linux. Event-sourced.
</p>

---

Vaultime is a desktop app that tracks real play sessions across PC games and launchers. It separates **runtime**, **active playtime** and **idle time**, stores everything locally, and shows your library in a dark, image-rich UI built from your own game artwork.

Remote backup is optional. It runs on a self-hosted, invite-only server (`api.codfishcloud.de`). There are no paid tiers or subscriptions.

## Features

- **Automatic session detection.** Polls for tracked game processes every 5 seconds and records sessions as they happen.
- **Active vs idle time.** Uses the foreground window and user input idle time (Win32 on Windows, X11 tools on Linux) to split active play from background time.
- **Event-sourced history.** Every session is an append-only, hash-chained event log. Totals are derived from it.
- **Honest trust labels.** Sessions are marked `Local`, `Suspicious` or `Recovered` based on clock comparisons, chain validation and crash recovery.
- **Sleep aware.** Time while the machine sleeps is skipped and logged instead of counted.
- **Game discovery.** Finds Steam games (registry based on Windows) and scans common launcher folders (Epic, GOG, Xbox, EA, Ubisoft, Battle.net, `~/Games`).
- **Local artwork.** Scans game folders for cover art, caches thumbnails and lets you pick the cover per game.
- **Local backups.** Self-contained exports with per-file checksums. Restores show a preview first and migrate older backups automatically.
- **Encrypted cloud backup.** Backups are encrypted on your device before upload. The passphrase never leaves the device.

## Platforms

| Platform | Status |
|---|---|
| Windows 10 and 11 | Supported, NSIS and MSI installers |
| Linux (X11) | Supported, deb, rpm and AppImage. Install `xprop` and `xprintidle` for foreground and idle detection |
| Linux (Wayland) | Partial. Detection goes through XWayland, which covers most Proton games but can misread native Wayland windows and idle time |
| macOS | Not built or tested yet |

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | [Tauri 2](https://v2.tauri.app/) |
| Core logic | Rust (edition 2024, toolchain pinned in `rust-toolchain.toml`) |
| Frontend | React 19, TypeScript 6, React Router 8 |
| Styling | Tailwind CSS 4 and [shadcn/ui](https://ui.shadcn.com/) on Base UI |
| Local database | SQLite through rusqlite (bundled, WAL mode) |
| Secrets | OS credential store (Windows Credential Manager, Secret Service on Linux) |
| Cloud API | Rust, Axum, PostgreSQL, filesystem blob storage |

## Cloud Backup Server

The API lives in [`apps/api`](apps/api). Deploy notes and scripts for the VPS are in [`deploy/vps`](deploy/vps/README.md), the design in [`docs/architecture`](docs/architecture).

Admins create invites in the app. To create one on the command line instead:

```bash
node scripts/generate-invite-key.mjs --count 3
```

## Getting Started

### Prerequisites

- Node.js 22.22 or newer (24 LTS recommended)
- Rust through [rustup](https://rustup.rs). The right version installs itself from `rust-toolchain.toml`.
- Windows: Visual Studio Build Tools with the "Desktop development with C++" workload. WebView2 ships with Windows 10 and 11.
- Linux:
  ```bash
  sudo apt-get install -y libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
    libayatana-appindicator3-dev librsvg2-dev
  ```
  Optional for better tracking on X11: `xprop` and `xprintidle`.

### Development

```bash
cd apps/desktop
npm ci
npm run tauri dev
```

### Checks

```bash
# apps/desktop
npm run typecheck && npm run lint && npm run build

# apps/desktop/src-tauri and apps/api
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

### Production Build

```bash
cd apps/desktop
npm run tauri build
```

Installers land in `apps/desktop/src-tauri/target/release/bundle/`.

### Releases

Pushing a `v*` tag builds Windows and Linux installers in GitHub Actions, uploads them with a signed `latest.json` for the auto-updater, and publishes the release. The `TAURI_SIGNING_PRIVATE_KEY` secret must be set. A manual run of the Release workflow builds installers without publishing.

## Project Structure

```
vaultime/
  apps/
    api/                    Self-hosted cloud backup API
    desktop/
      src/                  React frontend
        components/         Layout, charts, artwork, integrity badges, ui primitives
        features/           library, game-details, sessions, cloud, settings
        lib/                IPC wrappers, types, time and stat helpers
      src-tauri/src/        Rust core
        assets/             Artwork scanning and caching
        backup/             Local export and restore, encrypted cloud backup
        db/                 SQLite connection, migrations, repositories
        discovery/          Steam and folder discovery
        integrity/          Hash chains and trust validation
        platform/           Process list, foreground and idle detection per OS
        tracking/           Session engine and poll loop
        secure_storage.rs   OS credential store access
        commands.rs         Tauri IPC commands
  deploy/vps/               Caddy, systemd and setup scripts for the server
  docs/                     Architecture, legal pages, landing page
  scripts/                  Dev helpers
  assets/                   Brand assets
```

## License

MIT
