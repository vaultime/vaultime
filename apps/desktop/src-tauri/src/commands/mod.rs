// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tauri IPC command handlers.
//!
//! Commands that read the database or the disk use `#[tauri::command(async)]`,
//! so they run off the main thread and never freeze the window, not even while
//! another thread holds the database. Small writes stay on the main thread,
//! which runs them in the order the page sent them, so the last of two quick
//! changes wins. Cloud transfers and the OS credential store run on the
//! blocking pool, see `cloud::on_blocking_pool`.

pub mod app;
pub mod backups;
pub mod cloud;
pub mod discovery;
pub mod games;
pub mod pcs;
pub mod playtime;
pub mod sessions;
