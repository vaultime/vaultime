// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { MINUTE_MS } from "@/lib/constants";
import { at, session } from "@/test/sessions";
import type { Game } from "@/lib/types";
import { besideOthers, countedSession, playedToday, stepAside, stepsAside } from "./steps-aside";

function game(metadata: string): Game {
  return {
    id: "client",
    title: "Client",
    executable_path: null,
    install_folder: null,
    launcher_source: null,
    metadata_json: metadata,
    is_hidden: false,
    created_at: "",
    updated_at: "",
  };
}

describe("stepsAside", () => {
  it("reads the switch from the game's metadata", () => {
    expect(stepsAside(game('{"steps_aside":true}'))).toBe(true);
    expect(stepsAside(game("{}"))).toBe(false);
    expect(stepsAside(game("not json"))).toBe(false);
  });
});

describe("countedSession", () => {
  it("takes the time set aside out", () => {
    const client = session(at(2026, 10, 2, 18, 0), 180, {
      active_ms: 30 * MINUTE_MS,
      idle_ms: 150 * MINUTE_MS,
      set_aside_ms: 60 * MINUTE_MS,
      set_aside_active_ms: 0,
      set_aside_idle_ms: 60 * MINUTE_MS,
    });
    const counted = countedSession(client);
    expect([counted.runtime_ms, counted.active_ms, counted.idle_ms]).toEqual([
      120 * MINUTE_MS,
      30 * MINUTE_MS,
      90 * MINUTE_MS,
    ]);
  });
});

describe("stepAside", () => {
  const client = session(at(2026, 10, 2, 18, 0), 180, { game_id: "client", id: "client-session" });
  const match = session(at(2026, 10, 2, 19, 0), 60, { game_id: "match", id: "match-session" });

  it("keeps a client only where no other game ran", () => {
    const drawn = stepAside([client, match], new Set(["client"]));
    const parts = drawn.filter((part) => part.game_id === "client");
    expect(parts.map((part) => [part.started_at_wall, part.ended_at_wall])).toEqual([
      [at(2026, 10, 2, 18, 0), at(2026, 10, 2, 19, 0)],
      [at(2026, 10, 2, 20, 0), at(2026, 10, 2, 21, 0)],
    ]);
    // Its time spreads over the parts it kept.
    expect(parts.map((part) => part.runtime_ms)).toEqual([90 * MINUTE_MS, 90 * MINUTE_MS]);
    expect(drawn.filter((part) => part.game_id === "match")).toEqual([match]);
  });

  it("stays quick on a long history", () => {
    const many = Array.from({ length: 8000 }, (_, index) =>
      session(new Date(2016, 0, 1 + Math.floor(index / 2), 18 + (index % 2) * 2).toISOString(), 90, {
        game_id: index % 3 === 0 ? "client" : "match",
        id: `s-${index}`,
      }),
    );
    const started = performance.now();
    stepAside(many, new Set(["client"]));
    // A quadratic walk took seconds here, a binary search takes milliseconds.
    expect(performance.now() - started).toBeLessThan(500);
  });

  it("leaves everything alone without games that step aside", () => {
    expect(stepAside([client, match], new Set())).toEqual([client, match]);
  });
});

describe("besideOthers", () => {
  it("measures how long other games ran beside a game", () => {
    const client = session(at(2026, 10, 2, 18, 0), 120, { game_id: "client" });
    const match = session(at(2026, 10, 2, 18, 30), 90, { game_id: "match" });
    const { besideMs, share } = besideOthers("client", [client, match], new Set());
    expect(besideMs).toBe(90 * MINUTE_MS);
    expect(share).toBe(0.75);
  });
});

describe("playedToday", () => {
  it("spreads a session from yesterday the way the whole history would", () => {
    // Counted already, so its 180 minutes are the time it ran alone.
    const client = session(at(2026, 10, 1, 22, 0), 240, {
      game_id: "client",
      id: "client-session",
      runtime_ms: 180 * MINUTE_MS,
      active_ms: 180 * MINUTE_MS,
    });
    const match = session(at(2026, 10, 1, 22, 0), 60, { game_id: "match", id: "match-session" });
    const old = session(at(2026, 9, 1, 20, 0), 60, { game_id: "match", id: "old-session" });
    const now = new Date(2026, 9, 2, 12, 0);
    const today = playedToday([old, client, match], new Set(["client"]), now);
    expect(today).toBe(120 * MINUTE_MS);
  });
});
