// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import type { SessionEvent } from "@/lib/types";
import {
  formatIntegrityEventType,
  getIntegrityEventDetail,
  getIntegrityMeta,
  normalizeIntegrityStatus,
  parseIntegrityPayload,
} from "./integrity";

function event(event_type: string, payload: unknown = {}): SessionEvent {
  return {
    id: "event",
    session_id: "session",
    sequence: 2,
    event_type,
    event_time_wall: "2026-10-03T18:00:00.000Z",
    event_time_monotonic: null,
    payload_json: typeof payload === "string" ? payload : JSON.stringify(payload),
    hash_prev: null,
    hash_self: null,
    signature: null,
  };
}

describe("trust labels", () => {
  it("shows unknown or missing statuses as Local", () => {
    expect(normalizeIntegrityStatus(undefined)).toBe("local");
    expect(normalizeIntegrityStatus("  ")).toBe("local");
    expect(normalizeIntegrityStatus(" Suspicious ")).toBe("suspicious");
    expect(normalizeIntegrityStatus("verified")).toBe("local");
  });

  it("names each status and never claims more than a local record", () => {
    const labels = ["local", "recovered", "suspicious", "edited", "manual"].map(
      (status) => getIntegrityMeta(status).label,
    );
    expect(labels).toEqual(["Local", "Recovered", "Suspicious", "Edited", "Manual"]);
    expect(getIntegrityMeta("verified").label).toBe("Local");
    for (const status of ["local", "recovered", "suspicious", "edited", "manual"]) {
      expect(getIntegrityMeta(status).description).not.toMatch(/verified|guarantee|proof/i);
    }
  });
});

describe("session log", () => {
  it("names known events and spells out unknown ones", () => {
    expect(formatIntegrityEventType("tracking_gap")).toBe("Sleep or pause left out");
    expect(formatIntegrityEventType("added_manually")).toBe("Added by you");
    expect(formatIntegrityEventType("some_new_event")).toBe("Some new event");
  });

  it("reads only object payloads", () => {
    expect(parseIntegrityPayload("{not json")).toBeNull();
    expect(parseIntegrityPayload("3")).toBeNull();
    expect(parseIntegrityPayload("null")).toBeNull();
    expect(parseIntegrityPayload('{"reason":"x"}')).toEqual({ reason: "x" });
  });

  it("says why a session was flagged or recovered", () => {
    expect(getIntegrityEventDetail(event("integrity_flagged", { reason: "wall_clock_moved_backwards" }))).toBe(
      "The system clock went back",
    );
    expect(getIntegrityEventDetail(event("integrity_flagged", { reason: "wall_clock_step_mismatch" }))).toBe(
      "The system clock jumped between two checks",
    );
    expect(getIntegrityEventDetail(event("integrity_flagged", { reason: "wall_clock_drift_exceeded" }))).toBe(
      "The system clock ran too fast or too slow over the session",
    );
    expect(getIntegrityEventDetail(event("integrity_flagged", { reason: "odd_new_reason" }))).toBe("Odd new reason");
    expect(getIntegrityEventDetail(event("integrity_flagged", "{broken"))).toBe("Marked as suspicious");
    expect(getIntegrityEventDetail(event("recovered", { reason: "startup_orphan_cleanup" }))).toBe(
      "Closed when Vaultime started again",
    );
    expect(getIntegrityEventDetail(event("recovered"))).toBe("Closed after Vaultime was interrupted");
  });

  it("shows a correction with the old and new time and the reason", () => {
    const corrected = event("corrected", {
      previous: { runtime_ms: 2 * 3_600_000 + 8 * 60_000 },
      runtime_ms: 45 * 60_000,
      reason: "  Fell asleep ",
    });
    expect(getIntegrityEventDetail(corrected)).toBe("Changed from 2 h 08 to 45 min (“Fell asleep”)");
    expect(getIntegrityEventDetail(event("corrected", { reason: "" }))).toBe("Corrected");
    expect(getIntegrityEventDetail(event("added_manually", { reason: "On the laptop" }))).toBe(
      "Added by you (“On the laptop”)",
    );
    expect(getIntegrityEventDetail(event("added_manually", { reason: " " }))).toBe("Added by you");
  });

  it("tells how long a gap was and how far the clock was off", () => {
    expect(getIntegrityEventDetail(event("tracking_gap", { wall_gap_ms: 90 * 60_000 }))).toBe(
      "1 h 30 not counted while the PC slept or tracking paused",
    );
    expect(getIntegrityEventDetail(event("tracking_gap"))).toBe(
      "Time while the PC slept or tracking paused was not counted",
    );
    expect(getIntegrityEventDetail(event("heartbeat", { drift_ms: -3_400 }))).toBe(
      "Times saved, the system clock was 3s off",
    );
    expect(getIntegrityEventDetail(event("heartbeat", { drift_ms: 400 }))).toBe("Times saved");
    expect(getIntegrityEventDetail(event("heartbeat"))).toBe("Times saved");
    expect(getIntegrityEventDetail(event("started"))).toBe("First entry in the session's record");
    expect(getIntegrityEventDetail(event("ended"))).toBe("Session closed cleanly");
    expect(getIntegrityEventDetail(event("anything_else"))).toBe("Recorded");
  });
});
