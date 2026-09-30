// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { SECOND_MS } from "@/lib/constants";
import { formatHoursMinutes } from "@/lib/time";
import type { SessionEvent } from "@/lib/types";
import { capitalize } from "@/lib/words";

interface IntegrityMeta {
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
        description: "The system clock jumped or the record was changed outside Vaultime.",
      };
    case "recovered":
      return {
        label: "Recovered",
        description:
          "Vaultime was closed while the game ran, for example after a crash, and the session was closed on the next start.",
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
        description: "Recorded normally and unchanged since.",
      };
  }
}

function formatIntegrityReason(reason: string | null | undefined): string {
  switch (reason) {
    case "wall_clock_moved_backwards":
      return "The system clock went back";
    case "wall_clock_step_mismatch":
      return "The system clock jumped between two checks";
    case "wall_clock_drift_exceeded":
      return "The system clock ran too fast or too slow over the session";
    case "startup_orphan_cleanup":
      return "Closed when Vaultime started again";
    default:
      return capitalize((reason ?? "local_integrity_issue").replaceAll("_", " "));
  }
}

export function formatIntegrityEventType(eventType: string): string {
  switch (eventType) {
    case "started":
      return "Tracking started";
    case "heartbeat":
      return "Times saved";
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
      return "Added by you";
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
      return reason ?? "Marked as suspicious";
    case "recovered":
      return reason ?? "Closed after Vaultime was interrupted";
    case "started":
      return "First entry in the session's record";
    case "corrected": {
      const previous = payload?.previous as Record<string, unknown> | undefined;
      const before = typeof previous?.runtime_ms === "number" ? formatHoursMinutes(previous.runtime_ms) : null;
      const after = typeof payload?.runtime_ms === "number" ? formatHoursMinutes(payload.runtime_ms) : null;
      const why = typeof payload?.reason === "string" ? `: ${payload.reason}` : "";
      return before && after ? `Changed from ${before} to ${after}${why}` : `Corrected${why}`;
    }
    case "added_manually":
      return typeof payload?.reason === "string" && payload.reason
        ? `Added by you: ${payload.reason}`
        : "Added by you";
    case "ended":
      return "Session closed cleanly";
    case "tracking_gap": {
      const gapMs =
        typeof payload?.wall_gap_ms === "number" ? payload.wall_gap_ms : null;
      return gapMs === null
        ? "Time while the PC slept or tracking paused was not counted"
        : `${formatHoursMinutes(gapMs)} not counted while the PC slept or tracking paused`;
    }
    case "heartbeat": {
      const driftMs =
        typeof payload?.drift_ms === "number" ? payload.drift_ms : null;
      if (driftMs === null) {
        return "Times saved";
      }

      const driftSeconds = Math.round(Math.abs(driftMs) / SECOND_MS);
      return driftSeconds > 0
        ? `Times saved, the system clock was ${driftSeconds}s off`
        : "Times saved";
    }
    default:
      return "Recorded";
  }
}
