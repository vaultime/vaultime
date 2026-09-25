// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

/** Mirrors the Rust `Game` struct. */
export interface Game {
  id: string;
  title: string;
  executable_path: string | null;
  install_folder: string | null;
  launcher_source: string | null;
  metadata_json: string;
  is_hidden: boolean;
  created_at: string;
  updated_at: string;
}

/** Mirrors the Rust `CreateGame` struct. */
export interface CreateGameInput {
  title: string;
  executable_path?: string | null;
  install_folder?: string | null;
  launcher_source?: string | null;
}

/** Mirrors the Rust `UpdateGame` struct. */
export interface UpdateGameInput {
  title?: string | null;
  executable_path?: string | null;
  install_folder?: string | null;
  launcher_source?: string | null;
  is_hidden?: boolean | null;
}

/** Mirrors the Rust `Session` struct. */
export interface Session {
  id: string;
  game_id: string;
  device_id: string;
  started_at_wall: string;
  ended_at_wall: string | null;
  elapsed_monotonic_ms: number;
  active_ms: number;
  idle_ms: number;
  runtime_ms: number;
  integrity_status: string;
  closed_cleanly: boolean;
}

/** Mirrors the Rust `Setting` struct. */
export interface Setting {
  key: string;
  value: string;
  updated_at: string;
}
