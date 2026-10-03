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
// password dialog on the cloud page. Signed out, ?signin=empty|wrong|ok and
// ?signup=empty|short|invite|ok fill and send the forms of the cloud page.
// ?cover=add|adjust opens the cover crop dialog on a game page. ?sidebyside=1
// fills the three days before today with games that ran side by side.

import { mockIPC } from "@tauri-apps/api/mocks";
import { CLOUD_API_BASE_URL } from "@/lib/cloud-api";
import { DAY_MS, HOUR_MS, MINUTE_MS, SECOND_MS } from "@/lib/constants";
import { fillView, toCrop } from "@/lib/crop";
import type {
  ArtworkSource,
  CloudAuthSession,
  CloudBackupRecord,
  CloudDevice,
  CropRect,
  EarlierPlaytime,
  Game,
  GameAssetView,
  GameStatusChange,
  Session,
  SessionEvent,
  SteamPlaytimePreview,
  WindowSizeChoice,
  WindowSizeState,
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

if (params.get("sidebyside") === "1") {
  const midnight = new Date(now);
  midnight.setHours(0, 0, 0, 0);
  const dayStart = (daysAgo: number) =>
    new Date(midnight.getFullYear(), midnight.getMonth(), midnight.getDate() - daysAgo).getTime();
  const before = sessions.filter((session) => new Date(session.started_at_wall).getTime() < dayStart(3));
  sessions.splice(0, sessions.length, ...before);
  // [days ago, game index, start hour, hours]
  const sideBySide: [number, number, number, number][] = [
    // A launcher left open through three matches, like TFT in the League client.
    [1, 5, 18.95, 2.15],
    [1, 2, 19.03, 0.59],
    [1, 2, 19.62, 0.68],
    [1, 2, 20.43, 0.66],
    // Two separate pairs.
    [2, 3, 13, 2],
    [2, 4, 14, 2],
    [2, 1, 20, 3],
    [2, 2, 21, 1],
    // One game left running all day while others come and go.
    [3, 5, 9, 14],
    [3, 0, 11, 2.5],
    [3, 2, 13, 2],
    [3, 4, 17.5, 0.1],
    [3, 1, 19, 1],
    [3, 1, 20.2, 1],
  ];
  for (const [daysAgo, gameIndex, hour, hours] of sideBySide) {
    const start = dayStart(daysAgo) + hour * HOUR_MS;
    const runtime = Math.round(hours * HOUR_MS);
    const idle = Math.round(runtime * 0.1);
    sessions.push({
      id: `session-side-${daysAgo}-${gameIndex}-${hour}`,
      game_id: games[gameIndex].id,
      device_id: "preview",
      started_at_wall: iso(start),
      ended_at_wall: iso(start + runtime),
      elapsed_monotonic_ms: runtime,
      active_ms: runtime - idle,
      idle_ms: idle,
      runtime_ms: runtime,
      integrity_status: "local",
      closed_cleanly: true,
    });
  }
}

// A recent clock jump, so one card in the grid shows its badge.
const recentJump = sessions.findLast((session) => session.game_id === games[2].id);
if (recentJump) recentJump.integrity_status = "suspicious";

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

function addManual(gameId: string, startedAt: string, runtimeMs: number, reason: string, launcher: string | null = null): Session {
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
    payload_json: JSON.stringify({
      reason,
      started_at_wall: session.started_at_wall,
      ended_at_wall: session.ended_at_wall,
      launcher,
    }),
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
const MOCK_INVITE_CODE = "VTLINV-PREV-IEWA-BCDE-FGHJ-KLMN-PQRS";

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
      return cloudReply(200, cloudSession());
    case "/v1/auth/login":
      if (body.password !== MOCK_CLOUD_PASSWORD) {
        return cloudReply(401, { error: { code: "unauthorized", message: "invalid email or password" } });
      }
      return cloudReply(200, cloudSession());
    case "/v1/auth/signup":
      if (body.invite_code !== MOCK_INVITE_CODE) {
        return cloudReply(400, { error: { code: "bad_request", message: "invite code is invalid" } });
      }
      return cloudReply(201, cloudSession());
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

const signInStep = params.get("signin");
if (signInStep) {
  setTimeout(() => {
    if (signInStep !== "empty") {
      typeInto("cloud-login-email", "player@example.com");
      typeInto("cloud-login-password", signInStep === "wrong" ? "not my password" : MOCK_CLOUD_PASSWORD);
    }
  }, 800);
  setTimeout(() => clickButton("Sign in"), 1200);
}

const signUpStep = params.get("signup");
if (signUpStep) {
  setTimeout(() => {
    document.getElementById("cloud-signup-email")?.scrollIntoView();
    if (signUpStep !== "empty") {
      typeInto("cloud-signup-email", "player@example.com");
      typeInto("cloud-signup-password", signUpStep === "short" ? "short" : MOCK_CLOUD_PASSWORD);
      typeInto("cloud-signup-invite", signUpStep === "invite" ? "VTLINV-WRONG" : MOCK_INVITE_CODE);
      typeInto("cloud-signup-backup-passphrase", "a long backup passphrase");
      typeInto("cloud-signup-backup-passphrase-confirm", signUpStep === "short" ? "a long backup" : "a long backup passphrase");
    }
  }, 800);
  setTimeout(() => clickButton("Create account"), 1200);
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

// Appearance: ?theme=dark|light|system, ?ground=vault|graphite|midnight|moss|umber|garnet,
// ?accent= a swatch name or a hex color without the #, ?bg=1 for a sample
// background picture or ?bg=cover for dist-mock/covers/1.jpg, and ?dim= and
// ?blur= for its sliders. Changes on the settings page last until a reload.
const appearanceValues = new Map<string, string>();
for (const [param, key] of [
  ["theme", "appearance_mode"],
  ["ground", "appearance_ground"],
  ["dim", "background_dim"],
  ["blur", "background_blur"],
]) {
  const value = params.get(param);
  if (value) appearanceValues.set(key, value);
}
const accentParam = params.get("accent");
if (accentParam) appearanceValues.set("appearance_accent", /^[0-9a-f]{6}$/i.test(accentParam) ? `#${accentParam}` : accentParam);

/** A dusk landscape, standing in for a picture of the player's. */
function samplePicture(): string {
  const svg = `<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1600 1000' preserveAspectRatio='xMidYMid slice'>
    <defs><linearGradient id='sky' x1='0' y1='0' x2='0' y2='1'>
      <stop offset='0' stop-color='#1d2b64'/><stop offset='0.55' stop-color='#c2557a'/><stop offset='0.8' stop-color='#f8b26a'/>
    </linearGradient></defs>
    <rect width='1600' height='1000' fill='url(#sky)'/>
    <circle cx='1150' cy='560' r='120' fill='#ffd89b'/>
    <path d='M0 700 L260 480 L480 640 L760 380 L1040 620 L1300 460 L1600 640 L1600 1000 L0 1000Z' fill='#3b2a4f'/>
    <path d='M0 820 L340 640 L620 780 L900 600 L1220 800 L1600 700 L1600 1000 L0 1000Z' fill='#1b1430'/>
  </svg>`;
  return `data:image/svg+xml,${encodeURIComponent(svg)}`;
}

let backgroundPicture: string | null =
  params.get("bg") === "1" ? samplePicture() : params.get("bg") === "cover" ? "/covers/1.jpg" : null;

/** Answers the appearance commands, undefined for every other command. */
function appearanceIpc(cmd: string, args: Record<string, unknown>): unknown {
  switch (cmd) {
    case "set_setting":
      if (String(args.key).startsWith("appearance_") || String(args.key).startsWith("background_")) {
        appearanceValues.set(String(args.key), String(args.value));
      }
      return true;
    case "get_background_image":
      return backgroundPicture;
    case "set_background_image":
      backgroundPicture = samplePicture();
      return backgroundPicture;
    case "set_background_from_game": {
      const index = allGames.findIndex((game) => game.id === args.gameId);
      backgroundPicture = scenario === "covers" && index >= 0 ? `/covers/${index + 1}.jpg` : samplePicture();
      return backgroundPicture;
    }
    case "clear_background_image":
      backgroundPicture = null;
      return true;
    case "set_window_look":
      return true;
    default:
      return undefined;
  }
}

// Window size on a 1920 by 1080 screen with a taskbar. ?window=<choice> starts
// with another choice, like extra_large, and ?screen=1536x816 sets the room
// for the window.
const MOCK_TITLE_BAR_PX = 32;
const mockPresets = [
  { name: "compact", width: 1024, height: 640 },
  { name: "standard", width: 1280, height: 800 },
  { name: "large", width: 1536, height: 960 },
  { name: "extra_large", width: 1920, height: 1200 },
] as const;
const [mockScreenWidth, mockScreenHeight] = (params.get("screen") ?? "1920x1032").split("x").map(Number);
let mockWindowChoice = (params.get("window") ?? "standard") as WindowSizeChoice;
function mockWindowSize(): WindowSizeState {
  const presets = mockPresets.map((preset) => ({
    ...preset,
    fits: preset.width <= mockScreenWidth && preset.height + MOCK_TITLE_BAR_PX <= mockScreenHeight,
  }));
  const chosen = presets.findIndex((preset) => preset.name === mockWindowChoice);
  const applied = presets.slice(0, chosen + 1).filter((preset) => preset.fits).at(-1)?.name ?? "free";
  return { choice: mockWindowChoice, applied: chosen < 0 ? "free" : applied, presets };
}

// Cover crop on a game page. ?cover=add picks a wide sample logo, see-through
// unless ?art=photo picks an opaque landscape, and ?cover=adjust frames the
// cover in use (with ?mock=covers). ?save=1 then presses "Use as cover".
// Saving draws the crop the way the core cuts it.
interface MockArt {
  image: CanvasImageSource;
  width: number;
  height: number;
  source: ArtworkSource;
}
const pickedCovers = new Map<string, { asset: GameAssetView; art: MockArt; crop: CropRect }>();
let pickedArt: MockArt | null = null;

function canvas(width: number, height: number) {
  const element = document.createElement("canvas");
  element.width = width;
  element.height = height;
  return { element, context: element.getContext("2d")! };
}

function sampleArt(photo: boolean): MockArt {
  const { element, context } = canvas(1600, photo ? 900 : 520);
  if (photo) {
    const sky = context.createLinearGradient(0, 0, 0, 900);
    sky.addColorStop(0, "#2b3f6b");
    sky.addColorStop(0.6, "#d9825b");
    context.fillStyle = sky;
    context.fillRect(0, 0, 1600, 900);
    context.fillStyle = "#f4d58d";
    context.beginPath();
    context.arc(1050, 520, 90, 0, Math.PI * 2);
    context.fill();
    context.fillStyle = "#1d2236";
    context.beginPath();
    context.moveTo(0, 900);
    context.lineTo(0, 600);
    context.lineTo(380, 380);
    context.lineTo(720, 640);
    context.lineTo(1100, 420);
    context.lineTo(1600, 700);
    context.lineTo(1600, 900);
    context.fill();
  } else {
    context.fillStyle = "#e8d6a6";
    context.font = "600 210px Georgia, serif";
    context.textAlign = "center";
    context.textBaseline = "middle";
    context.fillText("ELDEN RING", 800, 270);
    context.fillRect(260, 420, 1080, 6);
  }
  return mockArt(element, element.width, element.height, photo);
}

function mockArt(image: CanvasImageSource, width: number, height: number, opaque: boolean, crop: CropRect | null = null): MockArt {
  const preview = canvas(width, height);
  preview.context.drawImage(image, 0, 0, width, height);
  const backdrop = canvas(90, 120);
  if (opaque) {
    const scale = Math.max(90 / width, 120 / height);
    backdrop.context.filter = "blur(3px) brightness(0.45)";
    backdrop.context.drawImage(image, (90 - width * scale) / 2, (120 - height * scale) / 2, width * scale, height * scale);
  } else {
    backdrop.context.fillStyle = "#26200f";
    backdrop.context.fillRect(0, 0, 90, 120);
  }
  return {
    image,
    width,
    height,
    source: {
      preview_data_url: preview.element.toDataURL(opaque ? "image/jpeg" : "image/png"),
      backdrop_data_url: backdrop.element.toDataURL("image/jpeg"),
      width,
      height,
      from_original: true,
      crop,
    },
  };
}

function drawCover(art: MockArt, crop: CropRect): string {
  const { element, context } = canvas(360, 480);
  const backdrop = new Image();
  backdrop.src = art.source.backdrop_data_url;
  context.imageSmoothingQuality = "high";
  context.drawImage(backdrop, 0, 0, 360, 480);
  const scale = 360 / (crop.width * art.width);
  context.drawImage(art.image, -crop.x * art.width * scale, -crop.y * art.height * scale, art.width * scale, art.height * scale);
  return element.toDataURL("image/jpeg");
}

function saveCover(gameId: string, art: MockArt, crop: CropRect) {
  pickedCovers.set(gameId, {
    art,
    crop,
    asset: {
      id: `picked-${gameId}`,
      game_id: gameId,
      asset_type: "cover",
      source: "user_picked",
      file_path: "C:/Users/you/Pictures/cover art.png",
      cache_path: null,
      hash: null,
      created_at: iso(now),
      preview_data_url: drawCover(art, crop),
      is_preferred: true,
    },
  });
  return gameAssets(gameId);
}

// Cover groups on a game page. ?covers=both gives each game images the player
// added and images Vaultime found, ?covers=added and ?covers=found only one
// kind. ?delete=<n> presses the delete button of the n-th image, ?confirm=1
// then confirms, and ?focus=delete puts the keyboard focus on the first one.
const coverMix = params.get("covers");
const deletedAssets = new Set<string>();
const mixedCovers = new Map<string, GameAssetView[]>();

/** A plain poster with a word on it, for sample covers. */
function posterArt(ground: string, ink: string, word: string): MockArt {
  const { element, context } = canvas(600, 900);
  context.fillStyle = ground;
  context.fillRect(0, 0, 600, 900);
  context.fillStyle = ink;
  context.font = "700 96px system-ui, sans-serif";
  context.textAlign = "center";
  context.fillText(word, 300, 760);
  return mockArt(element, 600, 900, true);
}

function sampleCovers(gameId: string): GameAssetView[] {
  if (!coverMix) return [];
  let covers = mixedCovers.get(gameId);
  if (!covers) {
    const asset = (id: string, source: string, filePath: string, art: MockArt): GameAssetView => ({
      id: `${id}-${gameId}`,
      game_id: gameId,
      asset_type: "cover",
      source,
      file_path: filePath,
      cache_path: null,
      hash: null,
      created_at: iso(now),
      preview_data_url: drawCover(art, toCrop(fillView(art), art)),
      is_preferred: false,
    });
    const added = [
      asset("added-logo", "user_picked", "C:/Users/you/Pictures/logo.png", posterArt("#1d1530", "#e8d6a6", "LOGO")),
      asset("added-photo", "user_picked", "C:/Users/you/Pictures/key art.jpg", sampleArt(true)),
    ];
    const found = [
      asset("found-steam", "steam_cache", "C:/Steam/appcache/librarycache/library_600x900.jpg", posterArt("#0f3b4a", "#9fe3f0", "STEAM")),
      asset("found-folder", "scanned_local", "C:/Games/Game/art/background.jpg", posterArt("#3a1f12", "#f0b45a", "ART")),
    ];
    covers = coverMix === "added" ? added : coverMix === "found" ? found : [...added, ...found];
    mixedCovers.set(gameId, covers);
  }
  return covers;
}

function gameAssets(gameId: string): GameAssetView[] {
  const picked = pickedCovers.get(gameId)?.asset;
  const covers = scenario === "covers" ? coverAssets().filter((asset) => asset.game_id === gameId) : [];
  const listed = picked ? [picked, ...covers.map((asset) => ({ ...asset, is_preferred: false }))] : covers;
  const all = [...listed, ...sampleCovers(gameId)].filter((asset) => !deletedAssets.has(asset.id));
  // As in the core, one image is always the cover.
  if (all.length > 0 && !all.some((asset) => asset.is_preferred)) all[0] = { ...all[0], is_preferred: true };
  return all;
}

const deleteStep = params.get("delete");
if (deleteStep || params.get("focus") === "delete") {
  setTimeout(() => {
    const section = [...document.querySelectorAll("section")].find((candidate) => candidate.querySelector("h2")?.textContent === "Cover");
    const buttons = [...(section?.querySelectorAll<HTMLButtonElement>('button[aria-label^="Delete "]') ?? [])];
    if (deleteStep) buttons[Number(deleteStep) - 1]?.click();
    else buttons[0]?.focus();
  }, 900);
  // ?confirm=1 then answers the question with Delete.
  if (params.has("confirm")) {
    setTimeout(() => {
      [...document.querySelectorAll("button")].find((button) => button.textContent?.trim() === "Delete")?.click();
    }, 1500);
  }
}

function preferredAssets(): GameAssetView[] {
  return allGames.flatMap((game) => gameAssets(game.id).filter((asset) => asset.is_preferred));
}

async function openAsset(gameId: string, assetId: string): Promise<ArtworkSource> {
  const picked = pickedCovers.get(gameId);
  if (picked && picked.asset.id === assetId) return { ...picked.art.source, crop: picked.crop };
  const asset = gameAssets(gameId).find((candidate) => candidate.id === assetId);
  const image = new Image();
  image.src = asset?.preview_data_url ?? "";
  await image.decode();
  pickedArt = mockArt(image, image.naturalWidth, image.naturalHeight, true);
  return { ...pickedArt.source, from_original: false };
}

const coverStep = params.get("cover");
if (coverStep) {
  const press = (label: string) =>
    [...document.querySelectorAll("button")].find((button) => button.textContent?.trim() === label)?.click();
  setTimeout(() => press(coverStep === "adjust" ? "Adjust" : "Add image"), 700);
  if (params.has("save")) setTimeout(() => press("Use as cover"), 1500);
}

mockIPC((cmd, payload) => {
  const args = (payload ?? {}) as Record<string, unknown>;
  const appearanceAnswer = appearanceIpc(cmd, args);
  if (appearanceAnswer !== undefined) return appearanceAnswer;
  switch (cmd) {
    case "get_app_version":
      return "0.3.0";
    case "get_device_id":
      return "preview";
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
      return addManual(
        String(args.gameId),
        String(args.startedAt),
        Number(args.runtimeMs),
        String(args.reason ?? ""),
        typeof args.launcher === "string" ? args.launcher : null,
      );
    case "get_session_events_for_game": {
      const ids = new Set(allSessions.filter((session) => session.game_id === args.gameId).map((session) => session.id));
      return events.filter((event) => ids.has(event.session_id));
    }
    case "get_active_sessions":
      return running;
    case "list_settings":
      return [
        { key: "idle_threshold_seconds", value: "300", updated_at: iso(now) },
        ...[...appearanceValues].map(([key, value]) => ({ key, value, updated_at: iso(now) })),
      ];
    case "get_tracking_diagnostics":
      return { platform: "windows", running: true, foreground_detection: "win32_api", idle_detection: "win32_api", controller_detection: "xinput", controllers_connected: 1, poll_interval_seconds: 5 };
    case "load_cloud_session_secure":
      return signedIn ? JSON.stringify(cloudSession()) : null;
    case "has_cloud_backup_key_secure":
      return signedIn;
    case "tray_available":
      return true;
    case "get_window_size":
      return mockWindowSize();
    case "set_window_size":
      mockWindowChoice = args.choice as WindowSizeChoice;
      return mockWindowSize();
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
      return gameAssets(String(args.gameId));
    case "list_preferred_game_assets":
      return preferredAssets();
    case "plugin:dialog|open":
      return "C:/Users/you/Pictures/cover art.png";
    case "open_artwork_file":
      pickedArt = sampleArt(params.get("art") === "photo");
      return pickedArt.source;
    case "open_game_asset_source":
      return openAsset(String(args.gameId), String(args.assetId));
    case "import_game_asset":
      return pickedArt ? saveCover(String(args.gameId), pickedArt, args.crop as CropRect) : [];
    case "delete_game_asset": {
      const gameId = String(args.gameId);
      deletedAssets.add(String(args.assetId));
      if (pickedCovers.get(gameId)?.asset.id === args.assetId) pickedCovers.delete(gameId);
      return gameAssets(gameId);
    }
    case "crop_game_asset": {
      const gameId = String(args.gameId);
      const picked = pickedCovers.get(gameId);
      const art = picked && picked.asset.id === args.assetId ? picked.art : pickedArt;
      return art ? saveCover(gameId, art, args.crop as CropRect) : gameAssets(gameId);
    }
    default:
      // Everything else answers with an empty result.
      return cmd.startsWith("list_") || cmd.startsWith("get_") ? [] : null;
  }
});
