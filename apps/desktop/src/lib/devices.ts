// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { Device, Session } from "@/lib/types";

/** The name a PC goes by, or its id when it has none. */
export function pcName(devices: Device[], id: string): string {
  return devices.find((device) => device.id === id)?.name ?? id;
}

/** The name of the PC a session was recorded on, when that is not this PC. */
export function recordedElsewhere(session: Session, devices: Device[], thisDeviceId: string | null): string | null {
  if (!thisDeviceId || session.device_id === thisDeviceId) return null;
  return pcName(devices, session.device_id);
}

/** Whether sessions of the PC came here by merge, so only that PC can correct them. */
export function isMergedPc(devices: Device[], id: string): boolean {
  return devices.some((device) => device.id === id && device.merged_at !== null);
}
