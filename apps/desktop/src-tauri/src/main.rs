// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Vaultime desktop application entry point.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    vaultime_lib::run();
}
