# Vaultime

Working guide for this repo. Update it when the structure or the rules change.

## Product

Vaultime is a local-first desktop app that tracks PC game playtime across
launchers and standalone games. It should feel like a polished game library
with trustworthy session history. Remote backup is an optional invite-only
service that runs on our own VPS.

## Stack

- Desktop shell: Tauri 2
- Core: Rust, edition 2024
- Frontend: React, TypeScript, Tailwind CSS, shadcn/ui on Base UI
- Local storage: SQLite through rusqlite (bundled)
- Cloud API: Rust, Axum and PostgreSQL on `codfishcloud.de`

## Platforms

- Windows and Linux are both first-class. Every feature has to work on both
  and CI checks both.
- macOS code paths may stay, but nothing builds or tests them.
- Windows uses Win32 APIs for foreground and idle detection. Linux uses X11
  tools (`xprop`, `xprintidle`) and falls back to a CPU heuristic.

## Layout

```text
apps/desktop/     Tauri app, React in src/, Rust in src-tauri/
apps/api/         Self-hosted cloud backup API
deploy/vps/       Caddy, systemd and bootstrap scripts for the VPS
docs/             Architecture notes, legal pages, landing page (docs/site)
scripts/          Dev helpers such as invite key generation
assets/           Brand assets
```

## Commands

Frontend, from `apps/desktop`:

- `npm ci`
- `npm run tauri dev` starts the app
- `npm run lint` and `npm run build`

Rust, from `apps/desktop/src-tauri` or `apps/api`:

- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`

Linux from a Windows machine: `bash scripts/linux-check.sh` runs the Linux CI
checks for both crates in Docker.

Run the checks for everything you touched before calling work done, on both
Windows and Linux when platform code changed. CI treats warnings as errors.

The Rust version is pinned in `rust-toolchain.toml`. Bump it on purpose and fix
new clippy lints in the same change.

App logs: `%LOCALAPPDATA%\com.vaultime.app\logs` on Windows and
`~/.local/share/com.vaultime.app/logs` on Linux.

## Product rules

- The app stays fully usable offline. Cloud is an extra, never a dependency.
- Tracking logic lives in Rust. The frontend only presents data.
- Play history is event-sourced. Never rely on a single mutable total.
- Track and show runtime, active playtime and idle time separately.
- Anti-tamper is integrity scoring, not prevention. Never overstate local
  guarantees.
- Use honest trust labels: `Local`, `Suspicious`, `Recovered`. Reserve
  `Verified` for server-backed validation.
- No paywalls, subscriptions, tiers or billing. Cloud access is invite-only
  and the same for every account. Server limits exist only to stop abuse.

## Domain

- Core entities: `Game`, `GameAsset`, `Session`, `SessionEvent`, `Device`,
  `BackupSnapshot`.
- Session events are append-only and hash-chained. Totals are derived from
  them.
- Every schema change is a new migration. Never edit a shipped migration.
- Keep enough metadata to recalculate totals when tracking rules change.

## Tracking

- Match games conservatively by executable path, install folder and
  fingerprints.
- Use monotonic time for durations and compare it with wall-clock time to spot
  suspicious changes.
- Expect suspend and resume, crashes and unclear process exits.
- Keep debug visibility high. Tracking correctness is the biggest product risk.

## UX

- Premium and image-rich, not a background utility.
- Dark-first with a deep-purple, atmospheric look instead of stock component
  styling.
- Library, game detail and session timeline come first.
- Show integrity state clearly without exaggerated security claims.

## Scope

- Focus on local tracking, a strong UI, local artwork and backups.
- Defer social features, plugins, mobile apps and deep launcher integrations.

## Code style

- Comments are short and plain. Only write one when it says something the code
  cannot. No comment beats a comment that repeats the code.
- In comments, docs, UI copy and commit messages: no em or en dashes, no
  semicolons in sentences, no decorative banner comments and no filler words
  like "robust", "seamless", "leverage" or "comprehensive".
- Match the style of the surrounding code.
- Keep dependencies on current stable versions and drop unused ones.

## Git

- Never add `Co-Authored-By` trailers or any other AI attribution to commits
  or pull requests.
- Commit subjects use `<file or module>: <message>`, for example
  `desktop/tracking: skip time while the machine sleeps` or
  `CLAUDE.md: add commit rules`. The message is lowercase and imperative. A
  plain body is optional.
- Author and committer are the global git identity (the outlook.de address).
  Merge pull requests by fast-forwarding `main` locally and pushing. The
  GitHub merge buttons replace the committer.
- Never commit generated files such as `__pycache__`, build output or `.env`.
- Never commit anything private: passwords, keys, tokens, server IPs or
  personal data. Secrets go into GitHub Actions secrets or local files outside
  the repo.
