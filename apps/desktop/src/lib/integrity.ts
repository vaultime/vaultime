// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import type { Session, SessionEvent } from "@/lib/types";

export interface IntegrityMeta {
  label: string;
  description: string;
  className: string;
}

export interface IntegritySummary {
  overallStatus: string;
  totalCount: number;
  localCount: number;
  suspiciousCount: number;
  recoveredCount: number;
  openCount: number;
  note: string;
}

const STATUS_PRIORITY: Record<string, number> = {
  local: 1,
  recovered: 2,
  suspicious: 3,
};

export function normalizeIntegrityStatus(
  status: string | null | undefined,
): string {
  const normalized = status?.toLowerCase().trim();
  if (!normalized) {
    return "local";
  }

  return normalized in STATUS_PRIORITY ? normalized : "local";
}

export function getIntegrityMeta(status: string): IntegrityMeta {
  switch (normalizeIntegrityStatus(status)) {
    case "suspicious":
      return {
        label: "Suspicious",
        description:
          "Tracking found a clock jump, timing drift, or another local inconsistency.",
        className:
          "border-amber-400/35 bg-amber-500/10 text-amber-100 shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]",
      };
    case "recovered":
      return {
        label: "Recovered",
        description:
          "Session history was reconstructed after an interrupted shutdown or restart.",
        className:
          "border-sky-400/35 bg-sky-500/10 text-sky-100 shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]",
      };
    case "local":
    default:
      return {
        label: "Local",
        description:
          "Locally tracked session history with no currently detected integrity issue.",
        className:
          "border-emerald-400/35 bg-emerald-500/10 text-emerald-100 shadow-[inset_0_1px_0_rgba(255,255,255,0.03)]",
      };
  }
}

export function summarizeIntegrity(sessions: Session[]): IntegritySummary {
  let overallStatus = "local";
  let localCount = 0;
  let suspiciousCount = 0;
  let recoveredCount = 0;
  let openCount = 0;

  for (const session of sessions) {
    const normalized = normalizeIntegrityStatus(session.integrity_status);

    if (normalized === "suspicious") {
      suspiciousCount += 1;
    } else if (normalized === "recovered") {
      recoveredCount += 1;
    } else {
      localCount += 1;
    }

    if (!session.ended_at_wall) {
      openCount += 1;
    }

    if (
      STATUS_PRIORITY[normalized] > STATUS_PRIORITY[overallStatus]
    ) {
      overallStatus = normalized;
    }
  }

  return {
    overallStatus,
    totalCount: sessions.length,
    localCount,
    suspiciousCount,
    recoveredCount,
    openCount,
    note: buildIntegrityNote({
      totalCount: sessions.length,
      suspiciousCount,
      recoveredCount,
      openCount,
    }),
  };
}

export function formatIntegrityReason(reason: string | null | undefined): string {
  switch (reason) {
    case "wall_clock_moved_backwards":
      return "System clock moved backwards";
    case "wall_clock_step_mismatch":
      return "Wall clock jumped away from monotonic time";
    case "wall_clock_drift_exceeded":
      return "Wall-clock drift exceeded tolerance";
    case "startup_orphan_cleanup":
      return "Recovered after restart";
    default:
      return titleCaseWords((reason ?? "local_integrity_issue").replaceAll("_", " "));
  }
}

export function formatIntegrityEventType(eventType: string): string {
  switch (eventType) {
    case "started":
      return "Tracking Started";
    case "heartbeat":
      return "Heartbeat";
    case "integrity_flagged":
      return "Integrity Flagged";
    case "recovered":
      return "Recovered";
    case "ended":
      return "Tracking Ended";
    default:
      return titleCaseWords(eventType.replaceAll("_", " "));
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
    case "ended":
      return "Session closed cleanly";
    case "heartbeat": {
      const driftMs =
        typeof payload?.drift_ms === "number" ? payload.drift_ms : null;
      if (driftMs === null) {
        return "Timing counters persisted";
      }

      const driftSeconds = Math.round(Math.abs(driftMs) / 1000);
      return driftSeconds > 0
        ? `Timing counters persisted, ${driftSeconds}s drift`
        : "Timing counters persisted";
    }
    default:
      return "Audit event recorded";
  }
}

function buildIntegrityNote({
  totalCount,
  suspiciousCount,
  recoveredCount,
  openCount,
}: {
  totalCount: number;
  suspiciousCount: number;
  recoveredCount: number;
  openCount: number;
}): string {
  if (totalCount === 0) {
    return "Trust labels appear after the first tracked session.";
  }

  if (suspiciousCount > 0) {
    return `${suspiciousCount} session${suspiciousCount === 1 ? "" : "s"} flagged for clock mismatch or timing drift.`;
  }

  if (recoveredCount > 0) {
    return `${recoveredCount} session${recoveredCount === 1 ? "" : "s"} reconstructed after a restart or interrupted shutdown.`;
  }

  if (openCount > 0) {
    return `${openCount} live session${openCount === 1 ? "" : "s"} still open on this device.`;
  }

  return "All tracked sessions look internally consistent in local mode so far.";
}

function titleCaseWords(value: string): string {
  return value.replace(/\b\w/g, (match) => match.toUpperCase());
}
