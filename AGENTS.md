# Vaultime Agent Guide

This repository currently contains planning material only. Use this file as the working guide for future implementation work, and treat `PLAN.md` as supporting product context rather than the day-to-day execution checklist.

## Product Summary

Vaultime is a local-first desktop app for tracking PC game playtime across launchers and standalone games. The product should feel like a polished game library manager with trustworthy session history and an optional paid cloud sync tier later.

## Default Stack

- Desktop shell: Tauri 2
- Core/backend: Rust
- Frontend: React + TypeScript
- UI: Tailwind CSS + shadcn/ui
- Local storage: SQLite
- Cloud later: Supabase or Postgres-backed custom services

## Non-Negotiable Product Rules

- Keep the app fully usable offline. Cloud is an enhancement, not a dependency.
- Keep tracking logic in Rust. The frontend should stay presentation-focused.
- Use an event-sourced model for play history. Do not rely on a single mutable total-playtime counter.
- Track and expose three time concepts: runtime, active playtime, and idle/background time.
- Treat anti-tamper as integrity scoring, not absolute prevention. Never overstate local guarantees.
- Prefer honest trust labels such as `Local`, `Suspicious`, `Recovered`, and reserve stronger wording like `Verified` for cloud-backed validation.

## Recommended Initial Repository Shape

```text
vaultime/
  apps/
    desktop/
      src/
      src-tauri/
  packages/
    ui/
    config/
  docs/
    architecture/
    product/
    release/
  scripts/
  AGENTS.md
```

## Implementation Order

1. Scaffold the Tauri 2 desktop app with React and TypeScript.
2. Add Tailwind and shadcn/ui, then build a minimal navigation shell.
3. Add SQLite integration, migrations, and the initial domain model.
4. Implement manual game registration before broader discovery.
5. Implement process detection and session start/end tracking on one OS first.
6. Add active versus idle playtime logic.
7. Build the library, game details, and session history UI.
8. Add local artwork scanning and caching.
9. Add the integrity event log and trust indicators.
10. Add local backup export/import before cloud work.

## Domain Expectations

- Core entities should include `Game`, `GameAsset`, `Session`, `SessionEvent`, `Device`, and `BackupSnapshot`.
- Store session events append-only and derive totals from those records.
- Design the database with migrations from the start.
- Preserve enough metadata to recalculate totals when tracking rules change later.

## Tracking Guidance

- Match games conservatively using executable path, install folder, and fingerprints where available.
- Use monotonic time for elapsed duration wherever possible.
- Compare monotonic and wall-clock signals to detect suspicious changes.
- Plan for suspend/resume, crashes, and uncertain process termination instead of assuming perfect exits.
- Build debug visibility early because tracking correctness is the highest product risk.

## UX Guidance

- The app should feel premium and image-rich, not like a background utility.
- Prioritize the library view, game detail view, and session timeline early.
- Keep integrity state legible in the UI without making exaggerated security claims.

## Scope Discipline

- Focus MVP work on local tracking, strong UI, local assets, and local backup/export.
- Defer social features, plugin systems, mobile apps, and deep launcher integrations.
- Support one operating system well before broad cross-platform expansion.
