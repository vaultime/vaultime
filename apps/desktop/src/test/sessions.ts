// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { MINUTE_MS } from "@/lib/constants";
import type { Session, SessionEvent } from "@/lib/types";

/** Local wall time as the ISO string the core stores. Months count from 1. */
export function at(year: number, month: number, day: number, hour = 0, minute = 0): string {
  return new Date(year, month - 1, day, hour, minute).toISOString();
}

/** A closed session that started at `start` and ran for `minutes`. */
export function session(start: string, minutes: number, overrides: Partial<Session> = {}): Session {
  const runtime = minutes * MINUTE_MS;
  return {
    id: `session-${start}`,
    game_id: "game-1",
    device_id: "test",
    started_at_wall: start,
    ended_at_wall: new Date(new Date(start).getTime() + runtime).toISOString(),
    elapsed_monotonic_ms: runtime,
    active_ms: runtime,
    idle_ms: 0,
    runtime_ms: runtime,
    integrity_status: "local",
    closed_cleanly: true,
    ...overrides,
  };
}

export function event(sessionId: string, type: string, payload: Record<string, unknown>): SessionEvent {
  return {
    id: `${sessionId}-${type}`,
    session_id: sessionId,
    sequence: 1,
    event_type: type,
    event_time_wall: "2026-09-29T20:00:00Z",
    event_time_monotonic: null,
    payload_json: JSON.stringify(payload),
    hash_prev: null,
    hash_self: null,
    signature: null,
  };
}
