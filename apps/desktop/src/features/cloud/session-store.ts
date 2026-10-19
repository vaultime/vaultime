// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The cloud session and backup key as this PC keeps them, and how it names itself to the server.

import { CloudApiError, cloudPostJson } from "@/lib/cloud-api";
import { CLOUD_DEVICE_ID_STORAGE_KEY, CLOUD_SESSION_STORAGE_KEY, FALLBACK_DEVICE_ID_CHARS } from "@/lib/constants";
import {
  clearCloudBackupKeySecure,
  clearCloudSessionSecure,
  hasCloudBackupKeySecure,
  loadCloudSessionSecure,
  storeCloudBackupKeySecure,
  storeCloudSessionSecure,
} from "@/lib/tauri";
import { compareTimes } from "@/lib/time";
import type { CloudAuthSession, CloudBackupRecord } from "@/lib/types";
import { logFailure } from "@/lib/utils";

export async function loadPersistedSession(): Promise<CloudAuthSession | null> {
  try {
    const secureRaw = await loadCloudSessionSecure();
    if (secureRaw) {
      return parseStoredSession(secureRaw);
    }
  } catch {
    // Falls through to the older storage.
  }

  // Older builds kept the session in localStorage.
  const legacyRaw = window.localStorage.getItem(CLOUD_SESSION_STORAGE_KEY);
  if (!legacyRaw) {
    return null;
  }

  return parseStoredSession(legacyRaw);
}

export async function persistCloudSession(session: CloudAuthSession): Promise<void> {
  await storeCloudSessionSecure(JSON.stringify(session));
  window.localStorage.removeItem(CLOUD_SESSION_STORAGE_KEY);
}

/**
 * Forgets the session when the server ended it. The backup key stays, so
 * signing in again does not ask for the passphrase. Signing out removes both.
 */
export async function forgetStoredSession(): Promise<void> {
  await clearCloudSessionSecure();
  window.localStorage.removeItem(CLOUD_SESSION_STORAGE_KEY);
}

/** Ends a session a refresh returned after the player signed out. */
export function revokeLateSession(late: CloudAuthSession) {
  void cloudPostJson("/v1/auth/logout", { refresh_token: late.refresh_token }).catch(
    logFailure("signing out a session that was renewed after signing out"),
  );
}

export async function clearPersistedSessionStorage(accountId?: string): Promise<void> {
  await Promise.all([
    clearCloudSessionSecure(),
    accountId ? clearCloudBackupKeySecure(accountId) : Promise.resolve(true),
  ]);
  window.localStorage.removeItem(CLOUD_SESSION_STORAGE_KEY);
}

/**
 * The backups a new passphrase is checked against. Without a connection the
 * list is empty and the upload checks the key before it sends anything.
 */
export async function backupsForKeyCheck(list: () => Promise<CloudBackupRecord[]>): Promise<CloudBackupRecord[]> {
  try {
    return await list();
  } catch (error) {
    if (error instanceof CloudApiError && error.status === 0) {
      return [];
    }
    throw error;
  }
}

/**
 * Stores the passphrase's key after checking it against the newest cloud
 * backup, so a typo cannot start backups nobody can open.
 */
export async function persistBackupPassphrase(
  session: CloudAuthSession,
  backupPassphrase: string,
  backups: CloudBackupRecord[],
): Promise<void> {
  const newest = backups
    .filter((backup) => backup.status === "complete")
    .sort((a, b) => compareTimes(b.uploaded_at, a.uploaded_at))[0];
  await storeCloudBackupKeySecure(session.user.id, backupPassphrase.trim(), newest?.metadata_json?.key_check ?? null);
}

export async function ensureBackupKeyForSession(
  session: CloudAuthSession,
): Promise<void> {
  const ready = await hasCloudBackupKeySecure(session.user.id);
  if (!ready) {
    throw new Error(
      "Set or unlock your backup passphrase on this PC before using cloud backups.",
    );
  }
}

export function parseStoredSession(raw: string): CloudAuthSession | null {
  try {
    const parsed = JSON.parse(raw) as CloudAuthSession;
    if (
      !parsed ||
      typeof parsed.access_token !== "string" ||
      typeof parsed.refresh_token !== "string" ||
      !parsed.user
    ) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export function shouldClearPersistedSession(error: unknown): boolean {
  return error instanceof CloudApiError && error.status === 401;
}

export function getOrCreateClientDeviceId() {
  const existing = window.localStorage.getItem(CLOUD_DEVICE_ID_STORAGE_KEY);
  if (existing) {
    return existing;
  }

  // The fallback skips the "0." that Math.random().toString(36) starts with.
  const created =
    typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `vaultime-${Math.random().toString(36).slice(2, 2 + FALLBACK_DEVICE_ID_CHARS)}`;
  window.localStorage.setItem(CLOUD_DEVICE_ID_STORAGE_KEY, created);
  return created;
}

export function detectPlatform() {
  const userAgent = navigator.userAgent.toLowerCase();
  const platform = navigator.platform.toLowerCase();

  if (platform.includes("win") || userAgent.includes("windows")) {
    return "windows";
  }
  if (platform.includes("mac") || userAgent.includes("mac os")) {
    return "macos";
  }
  if (platform.includes("linux") || userAgent.includes("linux")) {
    return "linux";
  }
  return "unknown";
}

export function getDeviceName() {
  switch (detectPlatform()) {
    case "windows":
      return "Vaultime Windows Desktop";
    case "macos":
      return "Vaultime macOS Desktop";
    case "linux":
      return "Vaultime Linux Desktop";
    default:
      return "Vaultime Desktop";
  }
}

export function isExpired(timestamp: string, skewMs: number) {
  const value = Date.parse(timestamp);
  if (Number.isNaN(value)) {
    return true;
  }

  return value <= Date.now() + skewMs;
}
