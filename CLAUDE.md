# Vaultime

Working guide for this repo. Update it when the structure or the rules change.

This file is public and read by anyone who works on the repo with an AI agent.
Keep it free of personal or private information. Maintainer-only notes such as
infrastructure, credentials locations, open tasks and decisions live in a local,
gitignored `PLAN.md`. Read it first when it exists, and never commit it.

## Product

Vaultime is a local-first desktop app that tracks PC game playtime across
launchers and standalone games. It should feel like a polished game library
with trustworthy session history. Cloud backup is optional and runs on a
Vaultime server that players set up themselves, on a rented server or a NAS.
The project runs no cloud service. Its VPS only hosts the website and the
downloads.

## Stack

- Desktop shell: Tauri 2
- Core: Rust, edition 2024
- Frontend: React, TypeScript, Tailwind CSS, shadcn/ui on Base UI
- Local storage: SQLite through rusqlite (bundled)
- Cloud server: Rust, Axum and PostgreSQL, run by players with Docker Compose

## Platforms

- Windows and Linux are both first-class. Every feature has to work on both
  and CI checks both.
- macOS code paths may stay, but nothing builds or tests them.
- Windows uses Win32 APIs for foreground and idle detection. Linux talks to
  the X server directly (x11rb, XWayland included) and reads GNOME's idle
  monitor over D-Bus on Wayland, with a CPU heuristic as the fallback.
- Controller input counts as activity: XInput on Windows, the evdev devices
  of controllers on Linux. Never open keyboards or mice directly.

## Layout

```text
apps/desktop/     Tauri app, React in src/, Rust in src-tauri/
apps/api/         Cloud backup server that players run, with its Dockerfile
deploy/server/    Docker Compose setup of that server, for players
deploy/vps/       Install script of the website and downloads on our VPS
docs/             Legal pages, landing page and documentation (docs/site)
packaging/linux/  Linux build image, AppStream metadata and package smoke test
packaging/aur/    PKGBUILD of the vaultime-bin AUR package, made from the deb
packaging/winget/ winget manifests of the latest release
scripts/          Dev helpers: Linux checks, packaging, cloud round trip, site
                  deploy, release upload, logo, UI screenshots, social preview
assets/           Brand assets, the README screenshots and the social preview
```

## Commands

Frontend, from `apps/desktop`:

- `npm ci`
- `npm run tauri dev` starts the app. Debug builds keep their cloud sign-in
  in their own keyring entries, so trying one never signs out an installed
  Vaultime.
- `scripts/third-party-notices.mjs` writes `public/third-party-notices.txt`
  (gitignored) from `cargo metadata` and `package-lock.json`. Tauri runs it
  before every dev and build run, Settings, About shows it.
- `npm run typecheck`, `npm run lint`, `npm test` and `npm run build`
- Tests live next to the code as `*.test.ts` and run in six time zones (see
  `vitest.config.ts`) with a German locale, so code that mixes up local and
  UTC days fails somewhere. Component tests are `*.test.tsx`, render with
  Testing Library in happy-dom and run once. The logic tests share one module
  cache, so one that mocks a module imports it and the code under test after
  `vi.resetModules()`, as `cloud-api.test.ts` does.
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
- `bash scripts/cloud-e2e.sh` builds the server image, runs it with
  PostgreSQL in Docker and a cloud backup round trip of the desktop core
  against it. Run it after changes to the backup format or the backup API.
- `docker build -t vaultime-server apps/api` builds the server image.
  `.github/workflows/server-image.yml` publishes it to GHCR for x86 and ARM
  on a release tag, or as `edge` when run by hand.
- `bash scripts/aur-check.sh` builds the AUR package in Arch Linux, writes
  its `.SRCINFO`, installs it and checks its libraries. The package names
  the AUR in `/usr/lib/vaultime/package-manager`, so the app leaves updates
  to it.

For winget, `bash scripts/winget-manifest.sh <version>` points the manifests
in `packaging/winget` at a published release, and
`winget validate --manifest packaging/winget` checks them.

Run the checks for everything you touched before calling work done, on both
Windows and Linux when platform code changed. CI treats warnings as errors.

Keep the suites fast and the coverage high: small focused tests, shared
setup, and timing or large-history tests behind `#[ignore]` as in `perf.rs`.
Check the suite time after adding tests, and cover the edges of new logic,
above all in tracking and integrity code.

The Rust version is pinned in `rust-toolchain.toml`. Bump it on purpose and fix
new clippy lints in the same change.

App logs: `%LOCALAPPDATA%\com.vaultime.app\logs` on Windows and
`~/.local/share/com.vaultime.app/logs` on Linux.

## Product rules

- The app stays fully usable offline. Cloud is an extra, never a dependency.
- Silent in the background. No notifications, toasts, reminders or windows
  that open by themselves. Vaultime shows things only when the player opens
  it. Only a database that does not open stops it with a message.
- Tracking logic lives in Rust. The frontend only presents data.
- Play history is event-sourced. Never rely on a single mutable total.
- Track and show runtime, active playtime and idle time separately.
- Anti-tamper is integrity scoring, not prevention. Never overstate local
  guarantees.
- Use honest trust labels: `Local`, `Suspicious`, `Recovered`, and `Edited`
  or `Manual` for changes the player made. A correction is an event with the
  old values and a reason, never an overwrite without a trace. Reserve
  `Verified` for server-backed validation.
- No paywalls, subscriptions, tiers or billing. The project runs no cloud
  service. A server a player runs treats every account the same, and its
  limits only keep one account from filling it. Its operator opens sign-ups
  with `VAULTIME_OPEN_SIGNUPS`. There are no invites, roles or admins.
- The app reaches a cloud server only through the core, over https or over
  plain http to a server on the same PC, so the window's content security
  policy names no outside address.

## Domain

- Core entities: `Game`, `GameAsset`, `Session`, `SessionEvent`, `Device`,
  `BackupSnapshot`.
- Session events are append-only and hash-chained. A session row keeps its
  totals, which have to match its newest timing event. The tracker writes a
  checkpoint on every change between active and idle and at least every
  30 s, so the time between two checkpoints is all of one kind.
- Stats come from play slices, quarter hours worked out from the events, and
  play spans, each session's stretches between checkpoints that the slices
  are spread from and that clock time counted once comes from. Both are a
  cache: rebuild them when counting rules change, never edit history to
  match a rule. Time counts on the day it happened, and time a launcher set
  aside for another game stays in the record but counts nowhere. The core
  works out every play time the pages show, so the journal and the stats
  agree.
- Each PC keeps a ledger, a hash chain signed with a key in `device-key`
  next to the database, which backups leave out. `device-keys` lists every
  public key the PC has signed with. It pins a session's chain at every
  event but checkpoints, a checkpoint each time the runtime enters a new
  `CHECKPOINT_PIN_INTERVAL_SECS` stretch, and notes removed sessions and
  restores. A session whose start names that interval may run at most one
  interval past its newest pinned checkpoint, and a recovery caps it there.
  On a PC
  only its own keys count, so every session there needs a pin of one of
  them: from its start, from when the ledger began, or from being taken in.
  Write session events only through `append_session_event`, which pins them.
- Sessions of another PC come in by merge, byte for byte, never hashed,
  pointed or signed again, as their game and PC are hashed in the start
  event. A session is taken in, with an `adopted` entry, only after it passes
  its check with the keys that count for the PC that recorded it
  (`ledger::Trust`): this PC's own for its sessions and for a backup that
  claims to be this PC, the keys trusted for a PC (those alone), for a PC
  seen without keys of its own those of the backup's PC if that one was
  trusted before, and the backup's keys for PCs never seen. Keys of a PC never seen are trusted on its first merge and
  recorded with `trusted` entries. Any other new key counts only when the
  player confirms it, which also vouches for its sessions that came in
  unvouched. From the cloud, a first merge counts only the keys that PC
  registered with the server, which keeps every key it was given, and the
  newer backups of a PC merged before come in by themselves only when they
  need no confirmation and no session fails its check. A backup must pass SQLite's integrity check and match the
  schema a fresh database gets, and a merge checks the rows and hashes it
  copied against the plan. A restore adopts only sessions that pass this
  way or that this PC's earlier pins confirm up to their newest timing
  event. After a session's first closing event only corrections follow.
  Never extend a chain recorded by a merged PC, and keep foreign games out
  of the tracker. Never close or recover a session another PC recorded: a
  running one from a restored backup stays open, out of the live views, and
  counts up to its last event until a merge brings its end.
- A fresh install (`ledger::fresh_install`: no sessions, no merged PC, a
  ledger of its own `began` entries only) that restores a backup of another
  PC with a sound ledger takes it over (`ledger::Takeover`): it continues as
  that PC, the keys the backup names for it count as its own beside its new
  key, written to `device-id` and `device-keys` and noted in a signed
  `took_over` entry. The player may decline it in the restore preview,
  where it is on by default, and the restore then runs as on a PC with
  history. A merge then takes in that PC's sessions it lacks only when the
  backup's PC trusted only keys this PC counts as its own for it. A PC with
  history never takes over.
- Games are linked, never merged. `game_links` maps a game to the game it
  counts as: another install on this PC, like a copy from a second launcher,
  which stays tracked, or a game of another PC. The map stays flat: a linked
  game is never a target, and linking a target moves its links along. The
  core shows linked games as the game they count as, while rules about a
  launcher, like Steam's earlier playtime, read the game that recorded a
  session.
- Every schema change is a new migration. Never edit a shipped migration.
- The API migrations 0001 to 0004 carry no SPDX lines and never change,
  because databases recorded their checksums. New migrations carry the
  lines.
- Keep enough metadata to recalculate totals when tracking rules change.

## Tracking

- Match games conservatively by executable path, install folder and
  fingerprints.
- Use monotonic time for durations and compare it with wall-clock time to spot
  suspicious changes.
- Expect suspend and resume, crashes and unclear process exits.
- Keep debug visibility high. Tracking correctness is the biggest product risk.

## UX

- Premium and image-rich while open, quiet while it runs in the
  background. Never ship stock component styling.
- Type: Mona Sans wide and heavy for titles (`font-display`, 125 % width,
  760), with page titles at 28px, the hero titles of Home and game pages at
  66px, 52px in windows under 1280px, and section titles at 18px. Newsreader,
  a serif made for screens, for sentences about play (`font-prose`). Geist
  for interface text, with tabular figures for durations, counts and stats,
  and the units smaller in big numbers. Geist Mono only for running clocks,
  times of day, chart hours, codes and paths. Durations read "4h 05m", "142h"
  and "40m". Labels are small and in sentence case (`label-small`), never
  spaced capitals.
- Surfaces: panels (`panel`) and cards with a faint 1px inner line, hairline
  rules inside them, no glows. Controls share one system: buttons 34px high
  with 8px corners and a faint fill, segmented choices (`segmented`,
  `segment`) and square step buttons. Colors come from the tokens in
  `index.css`, which `lib/theme.ts` derives for the chosen look, never loose
  hex values in components. The website in `docs/site` uses the same fonts,
  the graphite tokens with the grey accent of art without a strong color and
  classes of the same names. A guide takes the color of its picture, worked
  out by the rule of the app.
- The default look is lit by the game in front: the game a page is about,
  else the one the player picked for the other pages, else the one playing,
  else the one played last. Its art fills the window
  behind see-through panels, dimmed more where it would hide the text, and
  the accent, the color of active time, takes the strongest color of that
  art, a light grey for art without one. The window around the page and its
  icons match the page while it is seen, and show the game of Home while
  the window waits in the tray. The ground is
  graphite, a step above black, so the art carries the color. Players can
  pick light mode, another ground tone, a fixed accent, a picture of their
  own or no background. Every color derives from these choices in
  `lib/theme.ts`, so components use tokens only, and the `--violet` tokens
  mean the accent, whatever its color. Game pages take a tint from the cover
  art.
- Shell like a music player: the rail, the page and the live bar are panels
  on the darker window tone. The rail lists Home, Journal and Stats, the
  library with All games, and the games played last, with Cloud and Settings
  in its foot. A bar over the top of the page holds back, forward and the
  search that opens the Ctrl+K command palette. The live bar at the bottom
  takes a wash of the running game's colors.
- The window opens at a fixed preset size unless the player picks Free, so
  pages are laid out for those sizes, down to 1024x640, and still work in a
  free window down to 900x600.
- Write about play in plain sentences ("A long evening in Elden Ring"), not
  tables. Sessions read like a journal, with wording that varies from session
  to session but stays the same for each one.
- In the journal, each game gets a mark in the main color of its artwork,
  grey for black, white or grey art. Art with two strong colors shows both.
  A game whose first color looks like another game of the week or like the
  accent takes its second color, and a color shifts only when neither fits.
  Its title in the entries takes its mark, both colors for two-colored art.
- Logo: clock hands at five past eleven forming a V inside a rounded vault
  frame. The frame, the hands and the wordmark take the accent and the pivot
  the text color, and icons sit on a graphite tile. The logo files and the
  website show it as the app does where no game lends it a color, in the
  grey accent. The wordmark is Mona Sans 700 at full width
  (`font-wordmark`), and `scripts/build-brand.py` outlines it for the logo
  files. The lockups keep the proportions of the
  logo in the app, the mark a little taller than the letters. The app draws
  the tray, taskbar, title bar and shortcut icons from those files in the
  accent of the window, or the one picked.
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
- Never name other playtime trackers or competing apps in code, comments,
  docs, UI copy, the website or commit messages. Launchers and tools Vaultime
  reads data from may be named.
- No magic numbers. Every value that tunes behavior (durations, limits,
  thresholds, sizes, defaults) is a named constant in one file per codebase:
  `apps/desktop/src/lib/constants.ts`, `apps/desktop/src-tauri/src/constants.rs`
  and `apps/api/src/constants.rs`. Put the unit in the name (`_MS`, `_SECS`,
  `_BYTES`) and give each a one-line doc comment. A value used by both the
  frontend and the core says so in both files. Exempt are 0, 1 and plain
  arithmetic, values fixed by an outside format or API, tests and sample data,
  and styling, which lives in Tailwind classes and the theme in `index.css`.
- Every source file starts with the two SPDX lines of its neighbors. The
  server, the files in `apps/api` and `deploy/server`, is `AGPL-3.0-or-later`,
  everything else, the install script of the website in `deploy/vps`
  included, `GPL-3.0-or-later`. Outside code needs a license that fits both.
- Match the style of the surrounding code.
- Keep dependencies on current stable versions and drop unused ones.

## Docs

- `README.md` is for users only and stays short: pitch, screenshots, features,
  download and a link to the documentation. No build steps, stack tables,
  infrastructure or internal structure.
- The user documentation is one Markdown file per page in `docs/site/docs`,
  with its sidebar in `DOCS_NAV` in `scripts/build-site.py`. Keep it accurate
  to the code and never overstate what the trust checks guarantee.
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
- In a release, the release commit with the version bump and the dated
  `CHANGELOG.md` entry comes last. Re-create it when fixes come in. Push
  `main` and wait for CI before the tag.
- Ask the maintainer before every `v*` tag push. A tag publishes the update
  to every installed app and the server image, and a ruleset keeps `v*` tags
  from being moved or deleted.
- Never commit generated files such as `__pycache__`, build output or `.env`.
- Never commit anything private: passwords, keys, tokens, server IPs or
  personal data. Secrets go into GitHub Actions secrets or local files outside
  the repo.
