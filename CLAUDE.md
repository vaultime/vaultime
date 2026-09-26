# Vaultime

Working guide for this repo. Update it when the structure or the rules change.

This file is public and read by anyone who works on the repo with an AI agent.
Keep it free of personal or private information. Maintainer-only notes such as
infrastructure, credentials locations, open tasks and decisions live in a local,
gitignored `PLAN.md`. Read it first when it exists, and never commit it.

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
- Cloud API: Rust, Axum and PostgreSQL, self-hosted

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
deploy/vps/       Server install script, systemd unit and admin tools
docs/             Legal pages and landing page (docs/site)
packaging/linux/  Linux build image, AppStream metadata and package smoke test
scripts/          Dev helpers: Linux checks, packaging, API deploy, logo, UI screenshots
assets/           Brand assets
```

## Commands

Frontend, from `apps/desktop`:

- `npm ci`
- `npm run tauri dev` starts the app
- `npm run typecheck`, `npm run lint`, `npm test` and `npm run build`
- Tests live next to the code as `*.test.ts` and run in Berlin time with a
  German locale, so day boundaries and date formats differ from UTC.
- `VITE_MOCK_IPC=1 npx vite build --outDir dist-mock`, then
  `npx vite preview --outDir dist-mock` shows the UI in a browser with sample
  data. `bash ../../scripts/ui-screenshots.sh <folder> [page ...]` captures
  pages with headless Edge, `HEIGHT=1800` for long ones.

Rust, from `apps/desktop/src-tauri` or `apps/api`:

- `cargo fmt`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`

From a Windows machine, with Docker:

- `bash scripts/linux-check.sh` runs the Linux CI checks for both crates.
- `bash scripts/linux-package.sh` builds the Linux packages on Ubuntu 22.04
  and installs and starts them on Debian, Ubuntu, Fedora, Arch and openSUSE.

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

- Premium and image-rich, not a background utility. Never ship stock
  component styling.
- Editorial look: Fraunces (softened) for titles and sentences, Hanken
  Grotesk for interface text, JetBrains Mono for stats, clocks, times and
  paths. Small counts inside lists use Hanken Grotesk with tabular figures.
  Hairline rules instead of boxes and shadows, small uppercase labels,
  generous space. Colors come from the tokens in `index.css`, never loose hex
  values in components.
- Dark ink ground with violet `#9D7CFF` as the brand color and the color of
  active time. Game pages take a dark tint and light ink from the cover art.
- Shell like a music player: library rail on the left with search, a Ctrl+K
  command palette, and a live session bar at the bottom.
- Write about play in plain sentences ("A long evening in Elden Ring"), not
  tables. Sessions read like a journal.
- Logo: clock hands at five past eleven forming a V inside a rounded vault
  frame, with a violet pivot. The wordmark is Fraunces 600.
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
- No magic numbers. Every value that tunes behavior (durations, limits,
  thresholds, sizes, defaults) is a named constant in one file per codebase:
  `apps/desktop/src/lib/constants.ts`, `apps/desktop/src-tauri/src/constants.rs`
  and `apps/api/src/constants.rs`. Put the unit in the name (`_MS`, `_SECS`,
  `_BYTES`) and give each a one-line doc comment. A value used by both the
  frontend and the core says so in both files. Exempt are 0, 1 and plain
  arithmetic, values fixed by an outside format or API, tests and sample data,
  and styling, which lives in Tailwind classes and the theme in `index.css`.
- Match the style of the surrounding code.
- Keep dependencies on current stable versions and drop unused ones.

## Docs

- `README.md` is for users only: what Vaultime does, how to install it, how it
  counts playtime, where data lives, platform support. No build steps, stack
  tables, infrastructure or internal structure.
- Developer and maintainer information goes into `PLAN.md` (local, gitignored),
  or into `CLAUDE.md` when it is a general rule that is safe to publish.
- `CHANGELOG.md` records user-visible changes per release.

## Git

- Never add `Co-Authored-By` trailers or any other AI attribution to commits
  or pull requests.
- Commit subjects use `<file or module>: <message>`, for example
  `desktop/tracking: skip time while the machine sleeps` or
  `CLAUDE.md: add commit rules`. The message is lowercase and imperative. A
  plain body is optional.
- Author and committer are the local git identity. Merge pull requests by
  fast-forwarding `main` locally and pushing, because the GitHub merge buttons
  replace the committer.
- Never commit generated files such as `__pycache__`, build output or `.env`.
- Never commit anything private: passwords, keys, tokens, server IPs or
  personal data. Secrets go into GitHub Actions secrets or local files outside
  the repo.
