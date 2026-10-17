// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { Device, PcTakeover, Session } from "@/lib/types";

/** The name a PC goes by, or its id when it has none. */
export function pcName(devices: Device[], id: string): string {
  return devices.find((device) => device.id === id)?.name ?? id;
}

/** The name of the PC a session was recorded on, when that is not this PC. */
export function recordedElsewhere(session: Session, devices: Device[], thisDeviceId: string | null): string | null {
  if (!thisDeviceId || session.device_id === thisDeviceId) return null;
  return pcName(devices, session.device_id);
}

/** "This PC continues as Desktop.", for a restore that makes this PC continue as the PC of the backup. */
export function continuesAs(takeover: PcTakeover): string {
  return `This PC continues as ${takeover.name ?? "the PC that made this backup"}.`;
}

/** The choice to take over the PC of a backup: "This is Desktop, set up again". */
export function setUpAgain(takeover: PcTakeover | null): string {
  return takeover?.name
    ? `This is ${takeover.name}, set up again`
    : "This PC is the one that made this backup, set up again";
}

/** Said in place of the takeover note while the player declines it. */
export const TAKEOVER_DECLINED =
  "The history then comes in as another PC's, and a second PC is usually set up by merging that PC's backup under Settings, Other PCs instead.";

/** Whether sessions of the PC came here by merge, so only that PC can correct them. */
export function isMergedPc(devices: Device[], id: string): boolean {
  return devices.some((device) => device.id === id && device.merged_at !== null);
}
