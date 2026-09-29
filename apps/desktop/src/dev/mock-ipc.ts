// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Sample data for previewing the UI in a plain browser, without the Rust core.
// Only loaded when VITE_MOCK_IPC=1, never in real builds. Add ?mock=empty to
// the URL for a fresh install, ?mock=unplayed for games without sessions or
// ?mock=covers for artwork. Covers are not in the repo, copy portrait images
// to dist-mock/covers/<n>.jpg (n = 1 to 7) after building to see them.
// ?palette=1 opens the command palette. ?hidden=1 hides Celeste from the
// library. ?cloud=1 signs in to a fake cloud
// account, and ?password=open|wrong|short|mismatch|ok drives the change
// password dialog on the cloud page.

import { mockIPC } from "@tauri-apps/api/mocks";
import { CLOUD_API_BASE_URL } from "@/lib/cloud-api";
import { DAY_MS, HOUR_MS, MINUTE_MS, SECOND_MS } from "@/lib/constants";
import type {
  CloudAuthSession,
  CloudBackupRecord,
  CloudDevice,
  EarlierPlaytime,
  Game,
  GameStatusChange,
  Session,
  SessionEvent,
  SteamPlaytimePreview,
} from "@/lib/types";

const now = Date.now();
const iso = (ms: number) => new Date(ms).toISOString();

const params = new URLSearchParams(window.location.search);
const scenario = params.get("mock");
const signedIn = params.get("cloud") === "1" || params.has("password");

const titles =
  scenario === "covers"
    ? ["Balatro", "Hades II", "Slay the Spire 2", "Ghost of Tsushima Director's Cut", "Sun Haven", "Counter-Strike 2", "Crab Champions"]
    : ["Elden Ring", "Hades II", "Balatro", "Hollow Knight", "Celeste", "Stardew Valley", "Outer Wilds"];

const games: Game[] = titles.map((title, index) => ({
  id: `game-${index + 1}`,
  title,
  executable_path: `C:\\Games\\${title}\\${title.replace(/\s/g, "")}.exe`,
  install_folder: `C:\\Games\\${title}`,
  launcher_source: index < 4 ? "steam" : "folder_scan",
  metadata_json: "{}",
  is_hidden: params.get("hidden") === "1" && title === "Celeste",
  created_at: iso(now - 90 * DAY_MS),
  updated_at: iso(now - 90 * DAY_MS),
}));

// Over a year of weekday evenings and weekend afternoons, weighted towards the
// first games, with a quiet summer break and one game often left running.
const sessions: Session[] = [];
let seed = 7;
const random = () => {
  seed = (seed * 16807) % 2147483647;
  return seed / 2147483647;
};
const HISTORY_DAYS = 420;
const LEFT_RUNNING = "Stardew Valley";
for (let day = HISTORY_DAYS; day >= 1; day -= 1) {
  const weekday = new Date(now - day * DAY_MS).getDay();
  const weekend = weekday === 0 || weekday === 6;
  const summerBreak = day > 120 && day < 135;
  const chance = summerBreak ? 0.05 : weekend ? 0.85 : 0.6;
  const plays = random() < chance ? 1 + Math.floor(random() * (weekend ? 3 : 2)) : 0;
  for (let play = 0; play < plays; play += 1) {
    const game = games[Math.min(Math.floor(random() * random() * titles.length), titles.length - 1)];
    const firstHour = weekend ? 12 : 18;
    const start = now - day * DAY_MS + (firstHour + play * 2.5 + random()) * HOUR_MS - (now % DAY_MS);
    const runtime = Math.round((0.3 + random() * (weekend ? 4.2 : 3)) * HOUR_MS);
    const idleShare = game.title === LEFT_RUNNING ? 0.35 + random() * 0.2 : 0.05 + random() * 0.15;
    const idle = Math.round(runtime * idleShare);
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
      integrity_status: random() < 0.012 ? "suspicious" : random() < 0.01 ? "recovered" : "local",
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

// Steam's own count for three games. Balatro ran less on Steam than Vaultime
// tracked, so it adds nothing.
const trackedMs = (gameId: string) =>
  allSessions.filter((session) => session.game_id === gameId).reduce((sum, session) => sum + session.runtime_ms, 0);
const steamPreview: SteamPlaytimePreview = {
  found: true,
  account: "Player",
  games: [
    { gameId: "game-1", minutes: 38_000 },
    { gameId: "game-2", minutes: 16_500 },
    { gameId: "game-3", minutes: 900 },
  ]
    .map(({ gameId, minutes }) => {
      const tracked = trackedMs(gameId);
      return {
        game_id: gameId,
        title: games.find((game) => game.id === gameId)?.title ?? "",
        launcher_minutes: minutes,
        tracked_before_ms: tracked,
        earlier_ms: Math.max(0, minutes * MINUTE_MS - tracked),
        last_played_at: iso(now - 400 * DAY_MS),
      };
    })
    .sort((a, b) => b.earlier_ms - a.earlier_ms),
};
const toEarlier = (candidate: SteamPlaytimePreview["games"][number]): EarlierPlaytime => ({
  ...candidate,
  source: "steam",
  imported_at: iso(now - 2 * DAY_MS),
});
// Statuses and a note, so the journal and the library show them.
let statusChanges: GameStatusChange[] =
  scenario === "empty"
    ? []
    : [
        { id: "status-1", game_id: "game-5", status: "backlog", changed_at: iso(now - 30 * DAY_MS) },
        { id: "status-2", game_id: "game-6", status: "playing", changed_at: iso(now - 20 * DAY_MS) },
        { id: "status-3", game_id: "game-2", status: "finished", changed_at: iso(now - DAY_MS - 3 * HOUR_MS) },
        { id: "status-4", game_id: "game-7", status: "dropped", changed_at: iso(now - 9 * DAY_MS) },
      ];
const newestPlayed = allSessions.find((session) => session.ended_at_wall);
const sessionNotes: Record<string, string> = newestPlayed ? { [newestPlayed.id]: "Beat the boss on the third try." } : {};

let earlierPlaytime: EarlierPlaytime[] = params.get("earlier") === "1" ? steamPreview.games.map(toEarlier) : [];

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

/** A correction as the core writes it: the event keeps the old times and the reason. */
function correct(session: Session, timing: Pick<Session, "ended_at_wall" | "runtime_ms" | "active_ms" | "idle_ms">, reason: string) {
  events.push({
    id: `${session.id}-corrected-${events.length}`,
    session_id: session.id,
    sequence: 3,
    event_type: "corrected",
    event_time_wall: new Date().toISOString(),
    event_time_monotonic: null,
    payload_json: JSON.stringify({
      reason,
      runtime_ms: timing.runtime_ms,
      previous: { ended_at_wall: session.ended_at_wall, runtime_ms: session.runtime_ms },
    }),
    hash_prev: "preview",
    hash_self: "preview",
    signature: null,
  });
  Object.assign(session, timing, {
    elapsed_monotonic_ms: timing.runtime_ms,
    integrity_status: session.integrity_status === "suspicious" ? "suspicious" : "edited",
  });
  return session;
}

function addManual(gameId: string, startedAt: string, runtimeMs: number, reason: string): Session {
  const start = new Date(startedAt).getTime();
  const session: Session = {
    id: `session-manual-${start}`,
    game_id: gameId,
    device_id: "preview",
    started_at_wall: iso(start),
    ended_at_wall: iso(start + runtimeMs),
    elapsed_monotonic_ms: runtimeMs,
    active_ms: runtimeMs,
    idle_ms: 0,
    runtime_ms: runtimeMs,
    integrity_status: "manual",
    closed_cleanly: true,
  };
  allSessions.push(session);
  allSessions.sort((a, b) => b.started_at_wall.localeCompare(a.started_at_wall));
  events.push({
    id: `${session.id}-added`,
    session_id: session.id,
    sequence: 1,
    event_type: "added_manually",
    event_time_wall: session.ended_at_wall ?? session.started_at_wall,
    event_time_monotonic: null,
    payload_json: JSON.stringify({ reason, started_at_wall: session.started_at_wall, ended_at_wall: session.ended_at_wall }),
    hash_prev: null,
    hash_self: "preview",
    signature: null,
  });
  return session;
}

// One session cut short and one added by hand, so both labels show.
const leftRunning = allSessions.find(
  (session) => session.ended_at_wall && session.integrity_status === "local" && session.runtime_ms > 3 * HOUR_MS,
);
if (leftRunning?.ended_at_wall) {
  const end = new Date(leftRunning.ended_at_wall).getTime() - HOUR_MS;
  correct(
    leftRunning,
    {
      ended_at_wall: iso(end),
      runtime_ms: leftRunning.runtime_ms - HOUR_MS,
      active_ms: leftRunning.active_ms - Math.max(0, HOUR_MS - leftRunning.idle_ms),
      idle_ms: Math.max(0, leftRunning.idle_ms - HOUR_MS),
    },
    "Left it running while I cooked",
  );
}
if (scenario !== "empty" && scenario !== "unplayed") {
  addManual(games[1].id, iso(now - 2 * DAY_MS - 5 * HOUR_MS), 95 * MINUTE_MS, "On the Steam Deck");
}
const running = scenario === "empty" || scenario === "unplayed" ? [] : [live];

/** One cover per game for ?mock=covers, served from dist-mock/covers. */
function coverAssets() {
  return allGames.map((game, index) => ({
    id: `asset-${game.id}`,
    game_id: game.id,
    asset_type: "cover",
    source: "scanned_local",
    file_path: `covers/${index + 1}.jpg`,
    cache_path: null,
    hash: null,
    created_at: iso(now),
    preview_data_url: `/covers/${index + 1}.jpg`,
    is_preferred: true,
  }));
}

/** A result of the discovery scan, for the discover dialog. */
function discovered(title: string, path: string, source: string, alreadyAdded = false) {
  const installFolder = source === "folder_scan" ? null : path.slice(0, path.lastIndexOf("/"));
  return { title, executable_path: path, install_folder: installFolder, source, source_id: null, already_added: alreadyAdded };
}

const MOCK_CLOUD_PASSWORD = "correct horse battery";

function cloudSession(): CloudAuthSession {
  return {
    access_token: "preview",
    refresh_token: "preview",
    expires_at: iso(now + 15 * MINUTE_MS),
    refresh_expires_at: iso(now + 30 * DAY_MS),
    user: { id: "account-1", email: "player@example.com", role: params.get("admin") === "1" ? "admin" : "user" },
  };
}

function cloudBackup(id: string, label: string, daysAgo: number, games: number, sessionCount: number): CloudBackupRecord {
  const uploaded = iso(now - daysAgo * DAY_MS);
  return {
    id,
    label,
    storage_key: id,
    checksum: "preview",
    size_bytes: 180_000 + games * 9_000,
    backup_created_at: uploaded,
    uploaded_at: uploaded,
    status: "complete",
    client_device_id: "preview",
    metadata_json: {
      local_backup_id: id,
      backup_version: 1,
      created_at: uploaded,
      app_version: "0.1.0",
      source_device_id: "preview",
      overall_checksum: "preview",
      games_count: games,
      sessions_count: sessionCount,
      assets_count: games,
      asset_file_count: games,
      archive_format: "tar",
      encryption: "chacha20poly1305",
      archive_checksum: "preview",
      archive_size_bytes: 180_000,
      artwork_bytes: games * 1_200_000,
    },
  };
}

const cloudDevice: CloudDevice = {
  id: "device-1",
  client_device_id: "preview",
  device_name: "Vaultime Windows Desktop",
  platform: "windows",
  app_version: "0.1.0",
  registered_at: iso(now - 40 * DAY_MS),
  last_seen_at: iso(now),
};

function cloudReply(status: number, body: unknown) {
  return new Response(JSON.stringify(body), { status, headers: { "Content-Type": "application/json" } });
}

// Answers the cloud API in the browser, so the preview never reaches a server.
const realFetch = window.fetch.bind(window);
window.fetch = async (input, init) => {
  const url = new URL(input instanceof Request ? input.url : input.toString());
  if (url.origin !== new URL(CLOUD_API_BASE_URL).origin) return realFetch(input, init);
  const body = typeof init?.body === "string" ? (JSON.parse(init.body) as Record<string, string>) : {};
  switch (url.pathname) {
    case "/v1/auth/refresh":
    case "/v1/auth/login":
      return cloudReply(200, cloudSession());
    case "/v1/auth/password":
      if (body.current_password !== MOCK_CLOUD_PASSWORD) {
        return cloudReply(403, { error: { code: "forbidden", message: "the current password is wrong" } });
      }
      if ((body.new_password ?? "").length < 10) {
        return cloudReply(400, { error: { code: "bad_request", message: "password must be at least 10 characters long" } });
      }
      return cloudReply(200, cloudSession());
    case "/v1/devices/register":
      return cloudReply(200, cloudDevice);
    case "/v1/backups":
      return cloudReply(200, [cloudBackup("backup-2", "Before the reinstall", 2, 7, 38), cloudBackup("backup-1", "Backup", 16, 6, 24)]);
    case "/v1/admin/beta-applications":
      return cloudReply(200, [
        { id: "application-1", email: "sam@example.com", platform: "linux", note: "I play mostly through Heroic and Lutris.", created_at: iso(now - 2 * DAY_MS) },
        { id: "application-2", email: "robin@example.org", platform: "windows", note: null, created_at: iso(now - 5 * HOUR_MS) },
      ]);
    case "/v1/storage":
      return cloudReply(200, { backup_bytes: 480_000, artwork_bytes: 8_700_000, limit_bytes: 2048 * 1024 * 1024 });
    default:
      return cloudReply(404, { error: { code: "not_found", message: "not found" } });
  }
};

/** Types into a React controlled input. */
function typeInto(id: string, value: string) {
  const input = document.getElementById(id);
  if (!(input instanceof HTMLInputElement)) return;
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

function clickButton(text: string) {
  [...document.querySelectorAll("button")].find((button) => button.textContent?.trim() === text)?.click();
}

const passwordStep = params.get("password");
if (passwordStep) {
  setTimeout(() => clickButton("Change password"), 800);
  if (passwordStep !== "open") {
    const next = passwordStep === "short" ? "short" : "a brand new passphrase";
    setTimeout(() => {
      typeInto("cloud-current-password", passwordStep === "wrong" ? "not my password" : MOCK_CLOUD_PASSWORD);
      typeInto("cloud-new-password", next);
      typeInto("cloud-new-password-confirm", passwordStep === "mismatch" ? `${next}!` : next);
    }, 1200);
    setTimeout(() => {
      const dialog = document.querySelector("[role=dialog]");
      [...(dialog?.querySelectorAll("button") ?? [])].find((button) => button.textContent?.trim() === "Change password")?.click();
    }, 1600);
  }
}

// ?scan=1 presses "Start the scan" in the discover dialog, open it with ?discover=1.
if (new URLSearchParams(window.location.search).get("scan") === "1") {
  setTimeout(() => {
    const start = [...document.querySelectorAll("button")].find((button) => button.textContent?.includes("Start the scan"));
    start?.click();
  }, 800);
}

// ?palette=1 opens the command palette once the app has rendered.
if (new URLSearchParams(window.location.search).get("palette") === "1") {
  setTimeout(() => window.dispatchEvent(new KeyboardEvent("keydown", { key: "k", ctrlKey: true })), 500);
}

mockIPC((cmd, payload) => {
  const args = (payload ?? {}) as Record<string, unknown>;
  switch (cmd) {
    case "get_app_version":
      return "0.1.0";
    case "list_games":
      return allGames;
    case "list_earlier_playtime":
      return earlierPlaytime;
    case "list_status_changes":
      return statusChanges;
    case "set_game_status": {
      const change: GameStatusChange = {
        id: `status-${statusChanges.length + 1}`,
        game_id: String(args?.gameId),
        status: args?.status as GameStatusChange["status"],
        changed_at: new Date().toISOString(),
      };
      statusChanges = [...statusChanges, change];
      return change;
    }
    case "list_session_notes":
      return Object.entries(sessionNotes).map(([session_id, note]) => ({ session_id, note, updated_at: iso(now) }));
    case "set_session_note": {
      const id = String(args?.sessionId);
      const note = String(args?.note ?? "").trim();
      if (note) sessionNotes[id] = note;
      else delete sessionNotes[id];
      return note ? { session_id: id, note, updated_at: new Date().toISOString() } : null;
    }
    case "preview_steam_playtime":
      return steamPreview;
    case "import_steam_playtime":
      earlierPlaytime = steamPreview.games.map(toEarlier);
      return steamPreview;
    case "remove_steam_playtime": {
      const removed = earlierPlaytime.length;
      earlierPlaytime = [];
      return removed;
    }
    case "update_game": {
      const game = allGames.find((candidate) => candidate.id === args.id);
      const input = (args.input ?? {}) as { is_hidden?: boolean | null };
      if (game && typeof input.is_hidden === "boolean") game.is_hidden = input.is_hidden;
      return game ?? null;
    }
    case "list_sessions":
      // Copies, as the real IPC sends, so corrections show up as new data.
      return allSessions.map((session) => ({ ...session }));
    case "export_sessions":
      return allSessions.filter((session) => session.ended_at_wall).length;
    case "trim_session": {
      const session = allSessions.find((candidate) => candidate.id === args.sessionId);
      if (!session?.ended_at_wall) throw new Error("a running session cannot be corrected");
      const newEnd = new Date(String(args.endedAt)).getTime();
      const removed = Math.min(Math.max(0, new Date(session.ended_at_wall).getTime() - newEnd), session.runtime_ms);
      const idleCut = Math.min(removed, session.idle_ms);
      return correct(
        session,
        {
          ended_at_wall: iso(newEnd),
          runtime_ms: session.runtime_ms - removed,
          active_ms: session.active_ms - Math.min(removed - idleCut, session.active_ms),
          idle_ms: session.idle_ms - idleCut,
        },
        String(args.reason),
      );
    }
    case "discard_session": {
      const session = allSessions.find((candidate) => candidate.id === args.sessionId);
      if (!session?.ended_at_wall) throw new Error("a running session cannot be corrected");
      return correct(
        session,
        { ended_at_wall: session.ended_at_wall, runtime_ms: 0, active_ms: 0, idle_ms: 0 },
        String(args.reason),
      );
    }
    case "add_manual_session":
      return addManual(String(args.gameId), String(args.startedAt), Number(args.runtimeMs), String(args.reason ?? ""));
    case "get_session_events_for_game": {
      const ids = new Set(allSessions.filter((session) => session.game_id === args.gameId).map((session) => session.id));
      return events.filter((event) => ids.has(event.session_id));
    }
    case "get_active_sessions":
      return running;
    case "list_settings":
      return [{ key: "idle_threshold_seconds", value: "300", updated_at: iso(now) }];
    case "get_tracking_diagnostics":
      return { platform: "windows", running: true, foreground_detection: "win32_api", idle_detection: "win32_api", controller_detection: "xinput", controllers_connected: 1, poll_interval_seconds: 5 };
    case "load_cloud_session_secure":
      return signedIn ? JSON.stringify(cloudSession()) : null;
    case "has_cloud_backup_key_secure":
      return signedIn;
    case "tray_available":
      return true;
    case "set_cloud_signed_in":
      return null;
    case "get_auto_backup_folder":
      return "C:/Users/you/AppData/Roaming/com.vaultime.app/backups";
    case "discover_steam_games":
      return [
        discovered("Balatro", "C:/Steam/steamapps/common/Balatro/Balatro.exe", "steam", true),
        discovered("Slay the Spire 2", "C:/Steam/steamapps/common/Slay the Spire 2/SlayTheSpire2.exe", "steam"),
        // Real Steam paths get long, the list has to cut them.
        discovered(
          "Warhammer 40,000: Space Marine 2",
          "D:/SteamLibrary/steamapps/common/Space Marine 2/client_pc/root/bin/pc/Warhammer 40000 Space Marine 2 - Retail.exe",
          "steam",
        ),
        discovered("Ghost of Tsushima DIRECTOR'S CUT", "D:/SteamLibrary/steamapps/common/Ghost of Tsushima DIRECTOR'S CUT/GhostOfTsushima.exe", "steam"),
        discovered("Hades II", "D:/SteamLibrary/steamapps/common/Hades II/Ship/Hades2.exe", "steam"),
        discovered("Crab Champions", "D:/SteamLibrary/steamapps/common/Crab Champions/CrabChampions/Binaries/Win64/CrabChampions-Win64-Shipping.exe", "steam"),
        discovered("Counter-Strike 2", "D:/SteamLibrary/steamapps/common/Counter-Strike Global Offensive/game/bin/win64/cs2.exe", "steam"),
        discovered("Sun Haven", "D:/SteamLibrary/steamapps/common/Sun Haven/Sun Haven.exe", "steam"),
        discovered("Baldur's Gate 3", "D:/SteamLibrary/steamapps/common/Baldurs Gate 3/bin/bg3.exe", "steam"),
        discovered("Cyberpunk 2077", "D:/SteamLibrary/steamapps/common/Cyberpunk 2077/bin/x64/Cyberpunk2077.exe", "steam"),
      ];
    case "discover_launcher_games":
      return [
        discovered("Alan Wake 2", "C:/Epic Games/AlanWake2/AlanWake2.exe", "epic"),
        discovered("Stardew Valley", "C:/GOG Games/Stardew Valley/Stardew Valley.exe", "gog"),
        discovered("Diablo IV", "D:/BlizzardLibrary/Diablo IV/Diablo IV.exe", "battlenet"),
        discovered("Genshin Impact", "C:/Program Files/HoYoPlay/games/Genshin Impact game/GenshinImpact.exe", "hoyoplay"),
      ];
    case "get_default_scan_paths":
      return ["C:/Games"];
    case "discover_games":
      return [
        discovered("Some Indie Game", "C:/Games/Some Indie Game/Game.exe", "folder_scan"),
        // Inside the GOG game's folder, so the dialog leaves it out.
        discovered("StardewModdingAPI", "C:/GOG Games/Stardew Valley/StardewModdingAPI.exe", "folder_scan"),
      ];
    case "list_game_assets":
      return scenario === "covers" ? coverAssets().filter((asset) => asset.game_id === args.gameId) : [];
    case "list_preferred_game_assets":
      return scenario === "covers" ? coverAssets() : [];
    default:
      // Everything else answers with an empty result.
      return cmd.startsWith("list_") || cmd.startsWith("get_") ? [] : null;
  }
});
