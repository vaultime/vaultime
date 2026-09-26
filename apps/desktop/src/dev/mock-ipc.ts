// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

// Sample data for previewing the UI in a plain browser, without the Rust core.
// Only loaded when VITE_MOCK_IPC=1, never in real builds. Add ?mock=empty to
// the URL for a fresh install, or ?mock=unplayed for games without sessions.

import { mockIPC } from "@tauri-apps/api/mocks";
import { DAY_MS, HOUR_MS, MINUTE_MS, SECOND_MS } from "@/lib/constants";
import type { Game, Session, SessionEvent } from "@/lib/types";

const now = Date.now();
const iso = (ms: number) => new Date(ms).toISOString();

const scenario = new URLSearchParams(window.location.search).get("mock");

const titles = ["Elden Ring", "Hades II", "Balatro", "Hollow Knight", "Celeste", "Stardew Valley", "Outer Wilds"];

const games: Game[] = titles.map((title, index) => ({
  id: `game-${index + 1}`,
  title,
  executable_path: `C:\\Games\\${title}\\${title.replace(/\s/g, "")}.exe`,
  install_folder: `C:\\Games\\${title}`,
  launcher_source: index < 4 ? "steam" : "folder_scan",
  metadata_json: "{}",
  is_hidden: false,
  created_at: iso(now - 90 * DAY_MS),
  updated_at: iso(now - 90 * DAY_MS),
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
    const start = now - day * DAY_MS + (18 + play * 2.5 + random()) * HOUR_MS - (now % DAY_MS);
    const runtime = Math.round((0.6 + random() * 3.2) * HOUR_MS);
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

const liveRuntime = 84 * MINUTE_MS + 10 * SECOND_MS;
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
const played = scenario === "empty" || scenario === "unplayed" ? [] : [live, ...sessions];
const allSessions = played.sort((a, b) => b.started_at_wall.localeCompare(a.started_at_wall));
const allGames = scenario === "empty" ? [] : games;

// Flagged and recovered sessions explain themselves, and one skipped a sleep.
const events: SessionEvent[] = allSessions.flatMap((session, index) => {
  const event = (type: string, payload: Record<string, unknown>): SessionEvent => ({
    id: `${session.id}-${type}`,
    session_id: session.id,
    sequence: 2,
    event_type: type,
    event_time_wall: session.started_at_wall,
    event_time_monotonic: null,
    payload_json: JSON.stringify(payload),
    hash_prev: "preview",
    hash_self: "preview",
    signature: null,
  });
  if (session.integrity_status === "suspicious") return [event("integrity_flagged", { reason: "wall_clock_step_mismatch" })];
  if (session.integrity_status === "recovered") return [event("recovered", { reason: "startup_orphan_cleanup" })];
  if (index === 3) return [event("tracking_gap", { wall_gap_ms: 2 * HOUR_MS + 14 * MINUTE_MS })];
  return [];
});
const running = scenario ? [] : [live];

mockIPC((cmd, payload) => {
  const args = (payload ?? {}) as Record<string, unknown>;
  switch (cmd) {
    case "get_app_version":
      return "0.1.0";
    case "list_games":
      return allGames;
    case "get_game":
      return allGames.find((game) => game.id === args.id);
    case "list_sessions":
      return allSessions;
    case "get_sessions_for_game":
      return allSessions.filter((session) => session.game_id === args.gameId);
    case "get_session_events_for_game": {
      const ids = new Set(allSessions.filter((session) => session.game_id === args.gameId).map((session) => session.id));
      return events.filter((event) => ids.has(event.session_id));
    }
    case "get_active_sessions":
      return running;
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
