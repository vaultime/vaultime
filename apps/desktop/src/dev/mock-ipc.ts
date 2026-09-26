// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

// Sample data for previewing the UI in a plain browser, without the Rust core.
// Only loaded when VITE_MOCK_IPC=1, never in real builds.

import { mockIPC } from "@tauri-apps/api/mocks";
import type { Game, Session } from "@/lib/types";

const now = Date.now();
const iso = (ms: number) => new Date(ms).toISOString();
const HOUR = 3_600_000;
const DAY = 24 * HOUR;

const titles = ["Elden Ring", "Hades II", "Balatro", "Hollow Knight", "Celeste", "Stardew Valley", "Outer Wilds"];

const games: Game[] = titles.map((title, index) => ({
  id: `game-${index + 1}`,
  title,
  executable_path: `C:\\Games\\${title}\\${title.replace(/\s/g, "")}.exe`,
  install_folder: `C:\\Games\\${title}`,
  launcher_source: index < 4 ? "steam" : "folder_scan",
  metadata_json: "{}",
  is_hidden: false,
  created_at: iso(now - 90 * DAY),
  updated_at: iso(now - 90 * DAY),
}));

// A few weeks of evenings, weighted towards the first games.
const sessions: Session[] = [];
let seed = 7;
const random = () => {
  seed = (seed * 16807) % 2147483647;
  return seed / 2147483647;
};
for (let day = 21; day >= 1; day -= 1) {
  const plays = random() < 0.25 ? 0 : 1 + Math.floor(random() * 2);
  for (let play = 0; play < plays; play += 1) {
    const game = games[Math.min(Math.floor(random() * random() * titles.length), titles.length - 1)];
    const start = now - day * DAY + (18 + play * 2.5 + random()) * HOUR - (now % DAY);
    const runtime = Math.round((0.6 + random() * 3.2) * HOUR);
    const idle = Math.round(runtime * (0.05 + random() * 0.15));
    sessions.push({
      id: `session-${day}-${play}`,
      game_id: game.id,
      device_id: "preview",
      started_at_wall: iso(start),
      ended_at_wall: iso(start + runtime),
      elapsed_monotonic_ms: runtime,
      active_ms: runtime - idle,
      idle_ms: idle,
      runtime_ms: runtime,
      integrity_status: random() < 0.08 ? "suspicious" : random() < 0.06 ? "recovered" : "local",
      closed_cleanly: true,
    });
  }
}

const liveRuntime = 84 * 60_000 + 10_000;
const live: Session = {
  id: "session-live",
  game_id: games[0].id,
  device_id: "preview",
  started_at_wall: iso(now - liveRuntime),
  ended_at_wall: null,
  elapsed_monotonic_ms: liveRuntime,
  active_ms: Math.round(liveRuntime * 0.87),
  idle_ms: Math.round(liveRuntime * 0.13),
  runtime_ms: liveRuntime,
  integrity_status: "local",
  closed_cleanly: false,
};
const allSessions = [live, ...sessions].sort((a, b) => b.started_at_wall.localeCompare(a.started_at_wall));

mockIPC((cmd, payload) => {
  const args = (payload ?? {}) as Record<string, unknown>;
  switch (cmd) {
    case "get_app_version":
      return "0.1.0";
    case "list_games":
      return games;
    case "get_game":
      return games.find((game) => game.id === args.id);
    case "list_sessions":
      return allSessions;
    case "get_sessions_for_game":
      return allSessions.filter((session) => session.game_id === args.gameId);
    case "get_active_sessions":
      return [live];
    case "list_settings":
      return [{ key: "idle_threshold_seconds", value: "300", updated_at: iso(now) }];
    case "get_tracking_diagnostics":
      return { platform: "windows", running: true, foreground_detection: "win32_api", idle_detection: "win32_api", poll_interval_seconds: 5 };
    case "load_cloud_session_secure":
      return null;
    default:
      // Everything else answers with an empty result.
      return cmd.startsWith("list_") || cmd.startsWith("get_") ? [] : null;
  }
});
