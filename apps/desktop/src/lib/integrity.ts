// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { SECOND_MS } from "@/lib/constants";
import { formatHoursMinutes } from "@/lib/time";
import type { SessionEvent } from "@/lib/types";
import { capitalize } from "@/lib/words";

export interface IntegrityMeta {
  label: string;
  description: string;
}

const KNOWN_STATUSES = new Set(["local", "recovered", "suspicious", "edited", "manual"]);

export function normalizeIntegrityStatus(
  status: string | null | undefined,
): string {
  const normalized = status?.toLowerCase().trim();
  if (!normalized) {
    return "local";
  }

  return KNOWN_STATUSES.has(normalized) ? normalized : "local";
}

export function getIntegrityMeta(status: string): IntegrityMeta {
  switch (normalizeIntegrityStatus(status)) {
    case "suspicious":
      return {
        label: "Suspicious",
        description:
          "Tracking found a clock jump, timing drift, or another local inconsistency.",
      };
    case "recovered":
      return {
        label: "Recovered",
        description:
          "Session history was reconstructed after an interrupted shutdown or restart.",
      };
    case "edited":
      return {
        label: "Edited",
        description: "Tracked, then corrected by you. The old times and your reason stay in its history.",
      };
    case "manual":
      return {
        label: "Manual",
        description: "Added by you, not tracked. It counts as active time.",
      };
    case "local":
    default:
      return {
        label: "Local",
        description:
          "Locally tracked session history with no currently detected integrity issue.",
      };
  }
}

function formatIntegrityReason(reason: string | null | undefined): string {
  switch (reason) {
    case "wall_clock_moved_backwards":
      return "System clock moved backwards";
    case "wall_clock_step_mismatch":
      return "Wall clock jumped away from monotonic time";
    case "wall_clock_drift_exceeded":
      return "Wall clock drifted too far from monotonic time";
    case "startup_orphan_cleanup":
      return "Recovered after restart";
    default:
      return capitalize((reason ?? "local_integrity_issue").replaceAll("_", " "));
  }
}

export function formatIntegrityEventType(eventType: string): string {
  switch (eventType) {
    case "started":
      return "Tracking started";
    case "heartbeat":
      return "Heartbeat";
    case "integrity_flagged":
      return "Flagged";
    case "recovered":
      return "Recovered";
    case "ended":
      return "Tracking ended";
    case "tracking_gap":
      return "Sleep or pause left out";
    case "corrected":
      return "Corrected";
    case "added_manually":
      return "Added by hand";
    default:
      return capitalize(eventType.replaceAll("_", " "));
  }
}

export function parseIntegrityPayload(
  payloadJson: string,
): Record<string, unknown> | null {
  try {
    const parsed = JSON.parse(payloadJson) as unknown;
    return parsed && typeof parsed === "object"
      ? (parsed as Record<string, unknown>)
      : null;
  } catch {
    return null;
  }
}

export function getIntegrityEventDetail(event: SessionEvent): string {
  const payload = parseIntegrityPayload(event.payload_json);
  const reason =
    typeof payload?.reason === "string"
      ? formatIntegrityReason(payload.reason)
      : null;

  switch (event.event_type) {
    case "integrity_flagged":
      return reason ?? "Tracking mismatch recorded";
    case "recovered":
      return reason ?? "Session was reconstructed after an interruption";
    case "started":
      return "Local event chain opened for this session";
    case "corrected": {
      const previous = payload?.previous as Record<string, unknown> | undefined;
      const before = typeof previous?.runtime_ms === "number" ? formatHoursMinutes(previous.runtime_ms) : null;
      const after = typeof payload?.runtime_ms === "number" ? formatHoursMinutes(payload.runtime_ms) : null;
      const why = typeof payload?.reason === "string" ? `: ${payload.reason}` : "";
      return before && after ? `Changed from ${before} to ${after}${why}` : `Corrected${why}`;
    }
    case "added_manually":
      return typeof payload?.reason === "string" && payload.reason
        ? `Added by hand: ${payload.reason}`
        : "Added by hand";
    case "ended":
      return "Session closed cleanly";
    case "tracking_gap": {
      const gapMs =
        typeof payload?.wall_gap_ms === "number" ? payload.wall_gap_ms : null;
      return gapMs === null
        ? "Time while the machine slept or tracking paused was not counted"
        : `${formatHoursMinutes(gapMs)} not counted while the machine slept or tracking paused`;
    }
    case "heartbeat": {
      const driftMs =
        typeof payload?.drift_ms === "number" ? payload.drift_ms : null;
      if (driftMs === null) {
        return "Timing counters persisted";
      }

      const driftSeconds = Math.round(Math.abs(driftMs) / SECOND_MS);
      return driftSeconds > 0
        ? `Timing counters persisted, ${driftSeconds}s drift`
        : "Timing counters persisted";
    }
    default:
      return "Audit event recorded";
  }
}
