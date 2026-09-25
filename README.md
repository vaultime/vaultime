<p align="center">
  <img src="assets/timesafe.svg" alt="Vaultime icon" width="120" />
</p>

<p align="center">
  <img src="assets/vaultime.svg" alt="Vaultime" width="360" />
</p>

<p align="center">
  <strong>Universal game library and trustworthy playtime tracker.</strong><br />
  Local-first. Cross-platform. Event-sourced.
</p>

---

Vaultime is a cross-platform desktop application that tracks real play sessions across PC games and launchers. It distinguishes between **runtime**, **active playtime**, and **idle/background time**, stores everything locally by default, and presents your library in a premium, dark-themed UI with imported game artwork.

An invite-only cloud backup beta on your own VPS is the current direction for remote access. The design target is `codfishcloud.de` plus a custom backend with PostgreSQL, not public subscription billing.

## Features

- **Automatic session detection** — polls for tracked game processes and records sessions in real time.
- **Active vs idle playtime** — uses foreground-window and user-idle signals (X11, Win32, macOS) to separate active play from background/idle runtime.
- **Event-sourced tracking** — all play events are stored as append-only, hash-chained records, not just a total counter.
- **Integrity system** — sessions carry trust labels (Local, Suspicious, Recovered) derived from monotonic/wall-clock comparison, chain validation, and crash recovery state.
- **Local game artwork** — scans game folders for cover art, caches optimized thumbnails, and lets you pick preferred artwork per game.
- **Local backup & restore** — export self-contained snapshots with per-file checksums; restore with a full preview before overwriting.
- **Cross-platform tracking** — native activity signals on Linux (X11), Windows (Win32), and macOS (HID/osascript), with heuristic fallbacks.
- **Dark-first, deep-purple UI** — built with React, Tailwind CSS, and shadcn/ui for a premium game-library feel.

## Current Status

Milestones 1-14 cover the current prototype. The cloud access strategy is being revised away from public subscription billing toward invite-only access:

| Milestone | Status |
|---|---|
| Foundation / repo setup | Done |
| Local database and domain model | Done |
| Manual game registration | Done |
| Tracking engine v1 | Done |
| Active playtime logic | Done |
| Session history UI and stats | Done |
| Image import and asset pipeline | Done |
| Integrity system v1 | Done |
| Local backups and cross-platform support | Done |
| Cloud foundation prototype | Done |
| Cloud backup and sync prototype | Done |
| Invite-only cloud access design | In progress |
| Auto-discovery and launcher support | Done |
| Release engineering and publishing | Done* |

\* Code signing for Windows and macOS notarization require external certificates/credentials to be configured as CI secrets.

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop shell | [Tauri 2](https://v2.tauri.app/) |
| Backend / core logic | Rust |
| Frontend | React 19 + TypeScript |
| Styling | Tailwind CSS v4 + [shadcn/ui](https://ui.shadcn.com/) |
| Local database | SQLite (rusqlite, bundled, WAL mode) |
| Self-hosted cloud API | Rust + Axum + PostgreSQL + filesystem backup storage |
| Process detection | [sysinfo](https://crates.io/crates/sysinfo) |
| Image processing | [image](https://crates.io/crates/image) crate |

## Invite Keys

If you want to manually grant cloud-backup access, generate invite material with:

```bash
node scripts/generate-invite-key.mjs --count 3
```

The script prints the raw invite code plus the derived values you would store in
your server database. The intended server-side model is documented in
[`docs/architecture/invite-only-cloud.md`](docs/architecture/invite-only-cloud.md).

The concrete VPS service layout and starter database schema are documented in:

- [`docs/architecture/self-hosted-vps-stack.md`](docs/architecture/self-hosted-vps-stack.md)
- [`docs/architecture/self-hosted-postgres-schema.sql`](docs/architecture/self-hosted-postgres-schema.sql)

Tracked deploy templates for the VPS live in:

- [`deploy/vps/README.md`](deploy/vps/README.md)
- [`deploy/vps/Caddyfile`](deploy/vps/Caddyfile)
- [`deploy/vps/api.env.example`](deploy/vps/api.env.example)
- [`deploy/vps/vaultime-api.service`](deploy/vps/vaultime-api.service)

The self-hosted API scaffold lives in [`apps/api`](apps/api).

## Getting Started

### Prerequisites

- **Node.js** 20+
- **Rust** 1.85+ (2024 edition)
- **System dependencies** (Linux):
  ```
  sudo apt-get install -y libwebkit2gtk-4.1-dev libsoup-3.0-dev \
    libappindicator3-dev librsvg2-dev patchelf
  ```
- Optional for better tracking on Linux: `xprop`, `xprintidle`

### Development

```bash
# Install frontend dependencies
cd apps/desktop
npm install

# Run the app in dev mode (starts Vite + Tauri together)
npm run tauri dev
```

### Production Build

```bash
cd apps/desktop
npm run tauri build
```

Build artifacts are placed in `apps/desktop/src-tauri/target/release/bundle/`.

## Project Structure

```
vaultime/
  assets/                          # Brand assets (logos, icons)
  apps/
    api/                           # Self-hosted invite-only cloud API
      src/
    desktop/
      src/                         # React frontend
        components/
          charts/                  #   Activity charts
          layout/                  #   App shell, sidebar
          media/                   #   Game artwork display
          status/                  #   Integrity badges
          ui/                      #   shadcn/ui primitives
        features/
          library/                 #   Library page, game cards, add/edit/delete
          game-details/            #   Per-game detail view with stats
          sessions/                #   Session timeline and history
          cloud/                   #   Self-hosted cloud roadmap UI
          settings/                #   Tracking rules, detection status, backups
        lib/                       #   API layer, types, time/stat utilities
      src-tauri/                   # Rust backend
        src/
          assets/                  #   Folder scanning, thumbnail caching
          backup/                  #   Export/import/restore logic
          db/                      #   SQLite connection, migrations, repositories
          integrity/               #   Hash chains, trust validation
          platform/                #   OS-specific process/window/idle detection
          tracking/                #   Session engine, poll loop
          commands.rs              #   Tauri IPC command handlers
          error.rs                 #   Structured error types
          lib.rs                   #   App setup and state management
```

## License

MIT
