// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { invoke } from "@tauri-apps/api/core";

/** Temporary IPC test — calls the Rust `greet` command. */
export async function greet(name: string): Promise<string> {
  return invoke<string>("greet", { name });
}
