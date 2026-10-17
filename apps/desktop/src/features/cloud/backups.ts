// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { BYTES_PER_KIB, CLOUD_BACKUPS_CACHE_KEY_PREFIX, SIZE_ONE_DECIMAL_BELOW } from "@/lib/constants";
import { formatLongDate } from "@/lib/time";
import type { CloudBackupRecord } from "@/lib/types";

export function formatTimestamp(value: string | null | undefined) {
  if (!value) {
    return "Not set";
  }
  try {
    return formatLongDate(value);
  } catch {
    return value;
  }
}

export function formatByteSize(bytes: number) {
  if (bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const exponent = Math.min(Math.floor(Math.log(bytes) / Math.log(BYTES_PER_KIB)), units.length - 1);
  const value = bytes / BYTES_PER_KIB ** exponent;
  return `${value.toFixed(value >= SIZE_ONE_DECIMAL_BELOW || exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}

/** What a backup restores. Newer backups keep their artwork apart, stored once for all of them. */
export function backupSize(backup: CloudBackupRecord) {
  return formatByteSize(backup.size_bytes + (backup.metadata_json?.artwork_bytes ?? 0));
}

/** The PC a backup was made on, as the backup names it. */
export function backupSource(backup: CloudBackupRecord): string | null {
  return backup.metadata_json?.source_device_id ?? backup.client_device_id;
}

/** A local date and time field as an ISO timestamp, null when empty or unreadable. */
export function toIsoTimestamp(value: string) {
  if (!value) {
    return null;
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }

  return parsed.toISOString();
}

export function formatApiHostname(value: string) {
  try {
    return new URL(value).hostname;
  } catch {
    return value.replace(/^https?:\/\//, "");
  }
}

function cacheKey(accountId: string) {
  return `${CLOUD_BACKUPS_CACHE_KEY_PREFIX}${accountId}`;
}

/** The backups the server listed last time, shown while it cannot be reached. */
export function loadCachedBackups(accountId: string): CloudBackupRecord[] | null {
  try {
    const raw = window.localStorage.getItem(cacheKey(accountId));
    if (!raw) {
      return null;
    }

    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as CloudBackupRecord[]) : null;
  } catch {
    return null;
  }
}

export function persistCachedBackups(accountId: string, backups: CloudBackupRecord[]) {
  window.localStorage.setItem(cacheKey(accountId), JSON.stringify(backups));
}
