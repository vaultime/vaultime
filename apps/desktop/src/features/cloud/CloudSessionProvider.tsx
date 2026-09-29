// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import {
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { info, warn } from "@tauri-apps/plugin-log";
import {
  CLOUD_API_BASE_URL,
  CloudApiError,
  cloudDelete,
  cloudGetJson,
  cloudPostJson,
} from "@/lib/cloud-api";
import {
  CLOUD_AUTO_BACKUP_CHECK_MS,
  CLOUD_AUTO_BACKUP_INTERVAL_MS,
  CLOUD_DEVICE_ID_STORAGE_KEY,
  CLOUD_SESSION_STORAGE_KEY,
  FALLBACK_DEVICE_ID_CHARS,
  MIN_BACKUP_PASSPHRASE_CHARS,
  SETTING_KEYS,
  TOKEN_REFRESH_MARGIN_MS,
} from "@/lib/constants";
import { BACKUP_PASSPHRASE_TOO_SHORT } from "@/lib/cloud-api";
import {
  clearCloudBackupKeySecure,
  clearCloudSessionSecure,
  getAppVersion,
  hasCloudBackupKeySecure,
  listSettings,
  loadCloudSessionSecure,
  setCloudSignedIn,
  setSetting,
  restoreRemoteBackup as restoreRemoteBackupCommand,
  storeCloudBackupKeySecure,
  storeCloudSessionSecure,
  uploadRemoteBackup as uploadRemoteBackupCommand,
} from "@/lib/tauri";
import type {
  BetaApplication,
  CloudAdminInvite,
  CloudAuthSession,
  CloudBackupRecord,
  CloudCreateAdminInviteInput,
  CloudDevice,
  CloudStorage,
} from "@/lib/types";
import { createTokenRefresher } from "@/lib/token-refresh";
import { describeError } from "@/lib/utils";
import { CloudSessionContext, type CloudSessionContextValue } from "./cloud-context";

export function CloudSessionProvider({ children }: { children: ReactNode }) {
  const [initializing, setInitializing] = useState(true);
  const [session, setSession] = useState<CloudAuthSession | null>(null);
  const [device, setDevice] = useState<CloudDevice | null>(null);
  const [deviceError, setDeviceError] = useState<string | null>(null);
  const [backupKeyReady, setBackupKeyReady] = useState(false);
  const [autoBackup, setAutoBackupState] = useState(true);
  const sessionRef = useRef<CloudAuthSession | null>(null);
  // Counts sign-outs, so a refresh that finishes after one cannot sign back in.
  const signOutsRef = useRef(0);
  const bootstrappedRef = useRef(false);
  const appVersionRef = useRef<string | null>(null);
  const [refreshWithToken] = useState(() =>
    createTokenRefresher<CloudAuthSession>(
      (refreshToken) =>
        cloudPostJson<CloudAuthSession>("/v1/auth/refresh", {
          refresh_token: refreshToken,
        }),
      () => sessionRef.current,
      (next) => {
        sessionRef.current = next;
      },
    ),
  );
  const restoreDeviceRegistration = useEffectEvent(
    async (activeSession: CloudAuthSession) => {
      await tryRegisterDeviceForSession(activeSession);
    },
  );

  useEffect(() => {
    sessionRef.current = session;
  }, [session]);

  useEffect(() => {
    listSettings()
      .then((settings) => {
        const value = settings.find((setting) => setting.key === SETTING_KEYS.cloudAutoBackup)?.value;
        setAutoBackupState(value !== "false");
      })
      .catch(() => {});
  }, []);

  // Uploads a backup when the newest one on the server is a day old. Runs
  // while this PC is signed in with its backups unlocked.
  const backUpIfDue = useEffectEvent(async () => {
    try {
      const backups = await listBackups();
      const newest = backups
        .filter((backup) => backup.status === "complete")
        .reduce((latest, backup) => Math.max(latest, Date.parse(backup.uploaded_at)), 0);
      if (Date.now() - newest >= CLOUD_AUTO_BACKUP_INTERVAL_MS) {
        await uploadRemoteBackup("Automatic");
        void info("automatic cloud backup uploaded");
      }
    } catch (error) {
      // The next check tries again, the app works without the cloud.
      void warn(`automatic cloud backup failed: ${describeError(error)}`);
    }
  });

  // The tray and taskbar icon turn violet while signed in, like the logo.
  const signedIn = session !== null;
  useEffect(() => {
    setCloudSignedIn(signedIn).catch(() => {});
  }, [signedIn]);

  const accountId = session?.user.id ?? null;
  useEffect(() => {
    if (initializing || !accountId || !backupKeyReady || !autoBackup) return;
    void backUpIfDue();
    const timer = window.setInterval(() => void backUpIfDue(), CLOUD_AUTO_BACKUP_CHECK_MS);
    return () => window.clearInterval(timer);
  }, [initializing, accountId, backupKeyReady, autoBackup]);

  async function setAutoBackup(enabled: boolean) {
    setAutoBackupState(enabled);
    try {
      await setSetting(SETTING_KEYS.cloudAutoBackup, String(enabled));
    } catch (error) {
      setAutoBackupState(!enabled);
      throw error;
    }
  }

  useEffect(() => {
    if (bootstrappedRef.current) {
      return;
    }
    bootstrappedRef.current = true;

    let cancelled = false;

    async function restoreStoredSession() {
      const stored = await loadPersistedSession();
      if (!stored) {
        if (!cancelled) {
          setInitializing(false);
        }
        return;
      }

      applySession(stored);
      await syncBackupKeyState(stored);

      if (isExpired(stored.refresh_expires_at, 0)) {
        applyClearedSession();
        void forgetStoredSession();
        if (!cancelled) {
          setInitializing(false);
        }
        return;
      }

      const signOuts = signOutsRef.current;
      try {
        const refreshed = await refreshWithToken(stored.refresh_token);
        if (cancelled || signOuts !== signOutsRef.current) {
          revokeLateSession(refreshed);
          return;
        }
        applySession(refreshed);
        await persistCloudSession(refreshed);
        await syncBackupKeyState(refreshed);
        void restoreDeviceRegistration(refreshed);
      } catch (error) {
        if (!cancelled) {
          if (shouldClearPersistedSession(error)) {
            applyClearedSession();
            void forgetStoredSession();
          } else {
            setDeviceError(`The cloud server is not reachable: ${describeError(error)}`);
          }
        }
      } finally {
        if (!cancelled) {
          setInitializing(false);
        }
      }
    }

    void restoreStoredSession();

    return () => {
      cancelled = true;
    };
  }, [refreshWithToken]);

  function applySession(next: CloudAuthSession) {
    sessionRef.current = next;
    setSession(next);
  }

  function applyClearedSession() {
    sessionRef.current = null;
    setSession(null);
    setDevice(null);
    setDeviceError(null);
    setBackupKeyReady(false);
  }

  async function login(
    email: string,
    password: string,
    backupPassphrase?: string | null,
  ) {
    const next = await cloudPostJson<CloudAuthSession>("/v1/auth/login", {
      email,
      password,
    });
    await persistCloudSession(next);
    if (backupPassphrase?.trim()) {
      await persistBackupPassphrase(next, backupPassphrase);
      setBackupKeyReady(true);
    } else {
      await syncBackupKeyState(next);
    }
    applySession(next);
    void tryRegisterDeviceForSession(next);
    return next;
  }

  async function signUp(
    email: string,
    password: string,
    inviteCode: string,
    backupPassphrase: string,
  ) {
    const next = await cloudPostJson<CloudAuthSession>("/v1/auth/signup", {
      email,
      password,
      invite_code: inviteCode,
    });
    await persistCloudSession(next);
    await persistBackupPassphrase(next, backupPassphrase);
    setBackupKeyReady(true);
    applySession(next);
    void tryRegisterDeviceForSession(next);
    return next;
  }

  async function setBackupPassphrase(passphrase: string) {
    const current = sessionRef.current;
    if (!current) {
      throw new Error("Sign in before configuring a backup passphrase.");
    }

    const normalized = passphrase.trim();
    if (normalized.length < MIN_BACKUP_PASSPHRASE_CHARS) {
      throw new Error(BACKUP_PASSPHRASE_TOO_SHORT);
    }

    await persistBackupPassphrase(current, normalized);
    setBackupKeyReady(true);
  }

  // The server signs out every other device and returns a new session for this one.
  async function changePassword(currentPassword: string, newPassword: string) {
    const next = await withAuthenticatedSession((current) =>
      cloudPostJson<CloudAuthSession>(
        "/v1/auth/password",
        { current_password: currentPassword, new_password: newPassword },
        current.access_token,
      ),
    );
    await persistCloudSession(next);
    applySession(next);
  }

  async function logout() {
    signOutsRef.current += 1;
    const current = sessionRef.current;
    applyClearedSession();
    await clearPersistedSessionStorage(current?.user.id);

    if (!current) {
      return;
    }

    try {
      await cloudPostJson("/v1/auth/logout", {
        refresh_token: current.refresh_token,
      });
    } catch {
      // Local logout should still succeed if the server-side refresh token is already gone.
    }
  }

  async function refreshSession() {
    const current = sessionRef.current;
    if (!current) {
      return null;
    }

    if (isExpired(current.refresh_expires_at, 0)) {
      applyClearedSession();
      await forgetStoredSession();
      return null;
    }

    const signOuts = signOutsRef.current;
    try {
      const refreshed = await refreshWithToken(current.refresh_token);
      if (signOuts !== signOutsRef.current) {
        revokeLateSession(refreshed);
        return null;
      }
      applySession(refreshed);
      await persistCloudSession(refreshed);
      await syncBackupKeyState(refreshed);
      return refreshed;
    } catch (error) {
      if (shouldClearPersistedSession(error)) {
        applyClearedSession();
        await forgetStoredSession();
        return null;
      }

      setDeviceError(`The cloud server is not reachable: ${describeError(error)}`);
      return current;
    }
  }

  async function registerCurrentDevice() {
    const current = await ensureAuthenticatedSession();
    return registerDeviceForSession(current);
  }

  async function listBackups() {
    return withAuthenticatedSession((current) =>
      cloudGetJson<CloudBackupRecord[]>("/v1/backups", current.access_token),
    );
  }

  async function listBetaApplications() {
    return withAuthenticatedSession((current) =>
      cloudGetJson<BetaApplication[]>("/v1/admin/beta-applications", current.access_token),
    );
  }

  async function deleteBetaApplication(applicationId: string) {
    await withAuthenticatedSession((current) =>
      cloudDelete(`/v1/admin/beta-applications/${encodeURIComponent(applicationId)}`, current.access_token),
    );
  }

  async function getStorage() {
    return withAuthenticatedSession((current) =>
      cloudGetJson<CloudStorage>("/v1/storage", current.access_token),
    );
  }

  async function uploadRemoteBackup(label?: string | null) {
    return withAuthenticatedSession(async (current) => {
      await ensureBackupKeyForSession(current);
      const activeDeviceId = await ensureRemoteBackupDeviceId();
      return uploadRemoteBackupCommand(
        CLOUD_API_BASE_URL,
        current.access_token,
        current.user.id,
        activeDeviceId,
        label ?? null,
      );
    });
  }

  async function restoreRemoteBackup(backupId: string) {
    return withAuthenticatedSession(async (current) => {
      await ensureBackupKeyForSession(current);
      return restoreRemoteBackupCommand(
        CLOUD_API_BASE_URL,
        current.access_token,
        current.user.id,
        backupId,
      );
    });
  }

  async function deleteBackup(backupId: string) {
    return withAuthenticatedSession((current) =>
      cloudDelete(
        `/v1/backups/${encodeURIComponent(backupId)}`,
        current.access_token,
      ),
    );
  }

  async function createAdminInvite(input: CloudCreateAdminInviteInput) {
    const current = await ensureAuthenticatedSession();
    if (current.user.role !== "admin") {
      throw new Error("Admin access is required to generate invites.");
    }

    return withAuthenticatedSession((activeSession) =>
      cloudPostJson<CloudAdminInvite>(
        "/v1/admin/invites",
        {
          prefix: input.prefix,
          max_redemptions: input.max_redemptions,
          expires_at: input.expires_at,
          note: input.note,
        },
        activeSession.access_token,
      ),
    );
  }

  async function ensureAuthenticatedSession() {
    const current = sessionRef.current;
    if (!current) {
      throw new Error("Sign in to use cloud backup.");
    }

    if (isExpired(current.refresh_expires_at, 0)) {
      applyClearedSession();
      void forgetStoredSession();
      throw new Error("Your cloud session expired. Sign in again.");
    }

    if (isExpired(current.expires_at, TOKEN_REFRESH_MARGIN_MS)) {
      const signOuts = signOutsRef.current;
      try {
        const refreshed = await refreshWithToken(current.refresh_token);
        if (signOuts !== signOutsRef.current) {
          revokeLateSession(refreshed);
          throw new Error("You signed out of cloud backup.");
        }
        applySession(refreshed);
        await persistCloudSession(refreshed);
        return refreshed;
      } catch (error) {
        if (shouldClearPersistedSession(error)) {
          applyClearedSession();
          void forgetStoredSession();
          throw new Error("Your cloud session expired. Sign in again.", {
            cause: error,
          });
        }

        throw new Error(`The cloud server is not reachable: ${describeError(error)}`, {
          cause: error,
        });
      }
    }

    return current;
  }

  async function withAuthenticatedSession<T>(
    action: (activeSession: CloudAuthSession) => Promise<T>,
  ): Promise<T> {
    let current = await ensureAuthenticatedSession();

    try {
      return await action(current);
    } catch (error) {
      if (error instanceof CloudApiError && error.status === 401) {
        const refreshed = await refreshWithToken(current.refresh_token);
        applySession(refreshed);
        await persistCloudSession(refreshed);
        await syncBackupKeyState(refreshed);
        current = refreshed;
        return action(current);
      }

      throw error;
    }
  }

  async function registerDeviceForSession(activeSession: CloudAuthSession) {
    try {
      const appVersion = await getCachedAppVersion();
      const payload = {
        client_device_id: getOrCreateClientDeviceId(),
        device_name: getDeviceName(),
        platform: detectPlatform(),
        app_version: appVersion,
      };

      const registered = await cloudPostJson<CloudDevice>(
        "/v1/devices/register",
        payload,
        activeSession.access_token,
      );

      setDevice(registered);
      setDeviceError(null);
      return registered;
    } catch (error) {
      const message = describeError(error);
      setDeviceError(message);
      throw error;
    }
  }

  async function tryRegisterDeviceForSession(activeSession: CloudAuthSession) {
    try {
      return await registerDeviceForSession(activeSession);
    } catch {
      return null;
    }
  }

  async function ensureRemoteBackupDeviceId() {
    if (device?.client_device_id) {
      return device.client_device_id;
    }

    try {
      const registered = await registerCurrentDevice();
      return registered?.client_device_id ?? null;
    } catch {
      return null;
    }
  }

  async function getCachedAppVersion() {
    if (appVersionRef.current) {
      return appVersionRef.current;
    }

    try {
      appVersionRef.current = await getAppVersion();
    } catch {
      appVersionRef.current = "unknown";
    }

    return appVersionRef.current;
  }

  async function syncBackupKeyState(activeSession: CloudAuthSession) {
    try {
      const ready = await hasCloudBackupKeySecure(activeSession.user.id);
      setBackupKeyReady(ready);
      return ready;
    } catch {
      setBackupKeyReady(false);
      return false;
    }
  }

  const value: CloudSessionContextValue = {
    apiBaseUrl: CLOUD_API_BASE_URL,
    initializing,
    session,
    device,
    deviceError,
    backupKeyReady,
    autoBackup,
    setAutoBackup,
    isAdmin: session?.user.role === "admin",
    login,
    signUp,
    setBackupPassphrase,
    changePassword,
    logout,
    refreshSession,
    registerCurrentDevice,
    listBackups,
    getStorage,
    listBetaApplications,
    deleteBetaApplication,
    uploadRemoteBackup,
    restoreRemoteBackup,
    deleteBackup,
    createAdminInvite,
  };

  return (
    <CloudSessionContext.Provider value={value}>
      {children}
    </CloudSessionContext.Provider>
  );
}

async function loadPersistedSession(): Promise<CloudAuthSession | null> {
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

async function persistCloudSession(session: CloudAuthSession): Promise<void> {
  await storeCloudSessionSecure(JSON.stringify(session));
  window.localStorage.removeItem(CLOUD_SESSION_STORAGE_KEY);
}

/**
 * Forgets the session when the server ended it. The backup key stays, so
 * signing in again does not ask for the passphrase. Signing out removes both.
 */
async function forgetStoredSession(): Promise<void> {
  await clearCloudSessionSecure();
  window.localStorage.removeItem(CLOUD_SESSION_STORAGE_KEY);
}

/** Ends a session a refresh returned after the player signed out. */
function revokeLateSession(late: CloudAuthSession) {
  void cloudPostJson("/v1/auth/logout", { refresh_token: late.refresh_token }).catch(() => {});
}

async function clearPersistedSessionStorage(accountId?: string): Promise<void> {
  await Promise.all([
    clearCloudSessionSecure(),
    accountId ? clearCloudBackupKeySecure(accountId) : Promise.resolve(true),
  ]);
  window.localStorage.removeItem(CLOUD_SESSION_STORAGE_KEY);
}

/**
 * Stores the passphrase's key after checking it against the newest cloud
 * backup, so a typo cannot start backups nobody can open. Without a
 * connection the upload checks it instead.
 */
async function persistBackupPassphrase(
  session: CloudAuthSession,
  backupPassphrase: string,
): Promise<void> {
  const backups = await cloudGetJson<CloudBackupRecord[]>("/v1/backups", session.access_token).catch(
    () => [] as CloudBackupRecord[],
  );
  const newest = backups
    .filter((backup) => backup.status === "complete")
    .sort((a, b) => b.uploaded_at.localeCompare(a.uploaded_at))[0];
  await storeCloudBackupKeySecure(session.user.id, backupPassphrase.trim(), newest?.metadata_json?.key_check ?? null);
}

async function ensureBackupKeyForSession(
  session: CloudAuthSession,
): Promise<void> {
  const ready = await hasCloudBackupKeySecure(session.user.id);
  if (!ready) {
    throw new Error(
      "Set or unlock your backup passphrase on this PC before using cloud backups.",
    );
  }
}

function parseStoredSession(raw: string): CloudAuthSession | null {
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

function shouldClearPersistedSession(error: unknown): boolean {
  return error instanceof CloudApiError && error.status === 401;
}

function getOrCreateClientDeviceId() {
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

function detectPlatform() {
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

function getDeviceName() {
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

function isExpired(timestamp: string, skewMs: number) {
  const value = Date.parse(timestamp);
  if (Number.isNaN(value)) {
    return true;
  }

  return value <= Date.now() + skewMs;
}
