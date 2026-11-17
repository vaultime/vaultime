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
  BACKUP_PASSPHRASE_TOO_SHORT,
  CloudApiError,
  cloudDelete,
  cloudGetJson,
  cloudPostJson,
} from "@/lib/cloud-api";
import {
  CLOUD_AUTO_BACKUP_CHECK_MS,
  CLOUD_AUTO_BACKUP_INTERVAL_MS,
  MIN_BACKUP_PASSPHRASE_CHARS,
  SETTING_KEYS,
  TOKEN_REFRESH_MARGIN_MS,
  VAULTIME_URL,
} from "@/lib/constants";
import {
  clearCloudBackupKeySecure,
  discardCloudMerge as discardCloudMergeCommand,
  getAppVersion,
  getBackupDeviceId,
  hasCloudBackupKeySecure,
  isDevelopmentBuild,
  listCloudPcs as listCloudPcsCommand,
  mergeCloudBackup as mergeCloudBackupCommand,
  mergeDueCloudBackups,
  previewCloudMerge as previewCloudMergeCommand,
  setCloudSignedIn,
  restoreRemoteBackup as restoreRemoteBackupCommand,
  uploadRemoteBackup as uploadRemoteBackupCommand,
} from "@/lib/tauri";
import type {
  AutoMerge,
  CloudAuthSession,
  CloudBackupRecord,
  CloudDevice,
  CloudStorage,
  GameChoice,
} from "@/lib/types";
import { createTokenRefresher } from "@/lib/token-refresh";
import { describeError, logFailure } from "@/lib/utils";
import { useSettings } from "@/features/settings/settings-context";
import { CloudSessionContext, type CloudSessionContextValue } from "./cloud-context";
import {
  backupsForKeyCheck,
  clearPersistedSessionStorage,
  detectPlatform,
  ensureBackupKeyForSession,
  forgetStoredSession,
  getDeviceName,
  getOrCreateClientDeviceId,
  isExpired,
  loadPersistedSession,
  persistBackupPassphrase,
  persistCloudSession,
  revokeLateSession,
  shouldClearPersistedSession,
} from "./session-store";

/** Where a new sign-in goes. */
const DEFAULT_SERVER = VAULTIME_URL;

/** A session the server hands out, which remembers that server. */
async function sessionFrom(server: string, path: string, body: unknown, accessToken?: string): Promise<CloudAuthSession> {
  const answer = await cloudPostJson<Omit<CloudAuthSession, "server">>(server, path, body, accessToken);
  return { ...answer, server };
}

export function CloudSessionProvider({ children }: { children: ReactNode }) {
  const [bootstrapping, setBootstrapping] = useState(true);
  const [session, setSession] = useState<CloudAuthSession | null>(null);
  const [device, setDevice] = useState<CloudDevice | null>(null);
  const [deviceError, setDeviceError] = useState<string | null>(null);
  const [connectionError, setConnectionError] = useState<string | null>(null);
  const [backupKeyReady, setBackupKeyReady] = useState(false);
  const { values, read: settingsRead, save } = useSettings();
  // Unset on PCs from before the switch, until the first start decides.
  const cloudSetting = values[SETTING_KEYS.cloudBackup];
  const enabled = cloudSetting === "true";
  // Turned off, there is no saved session to look for.
  const initializing = cloudSetting !== "false" && bootstrapping;
  const [autoBackup, setAutoBackupState] = useState(() => values[SETTING_KEYS.cloudAutoBackup] !== "false");
  const [waitingMerges, setWaitingMerges] = useState<AutoMerge[]>([]);
  const [autoBackupError, setAutoBackupError] = useState<string | null>(null);
  const sessionRef = useRef<CloudAuthSession | null>(null);
  // Counts sign-outs, so a refresh that finishes after one cannot sign back in.
  const signOutsRef = useRef(0);
  const bootstrappedRef = useRef(false);
  // Set when the server could not be reached at start, so this PC registers
  // once a later renewal of the session works.
  const registerOnRenewalRef = useRef(false);
  const appVersionRef = useRef<string | null>(null);
  const [refreshWithToken] = useState(() =>
    createTokenRefresher<CloudAuthSession>(
      (stale) => sessionFrom(stale.server, "/v1/auth/refresh", { refresh_token: stale.refresh_token }),
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
    if (!registerOnRenewalRef.current || connectionError || !session) return;
    registerOnRenewalRef.current = false;
    void restoreDeviceRegistration(session);
  }, [connectionError, session]);


  // Uploads a backup when this PC's newest one on the server is a day old,
  // whatever the account's other PCs upload. Then the newer backups of PCs
  // merged here before come in. Runs while this PC is signed in with its
  // backups unlocked.
  const checkCloud = useEffectEvent(async () => {
    if (autoBackup) {
      try {
        const thisPc = await getBackupDeviceId();
        const backups = await listBackups();
        const newest = backups
          .filter((backup) => backup.status === "complete" && backup.metadata_json?.source_device_id === thisPc)
          .reduce((latest, backup) => Math.max(latest, Date.parse(backup.uploaded_at)), 0);
        if (Date.now() - newest >= CLOUD_AUTO_BACKUP_INTERVAL_MS) {
          await uploadRemoteBackup("Automatic");
          void info("automatic cloud backup uploaded");
        }
        setAutoBackupError(null);
      } catch (error) {
        // The next check tries again, the app works without the cloud.
        setAutoBackupError(describeError(error));
        void warn(`automatic cloud backup failed: ${describeError(error)}`);
      }
    }
    try {
      const done = await withAuthenticatedSession((current) =>
        mergeDueCloudBackups(current.server, current.access_token, current.user.id),
      );
      setWaitingMerges(done.filter((merge) => merge.outcome !== "merged"));
    } catch (error) {
      void warn(`automatic merge from the cloud failed: ${describeError(error)}`);
    }
  });

  // The tray and taskbar icon turn violet while signed in, like the logo.
  const signedIn = session !== null;
  useEffect(() => {
    setCloudSignedIn(signedIn).catch(logFailure("coloring the tray icon for cloud backup"));
  }, [signedIn]);

  const accountId = session?.user.id ?? null;
  useEffect(() => {
    if (initializing || !accountId || !backupKeyReady) return;
    const first = window.setTimeout(() => void checkCloud(), 0);
    const timer = window.setInterval(() => void checkCloud(), CLOUD_AUTO_BACKUP_CHECK_MS);
    return () => {
      window.clearTimeout(first);
      window.clearInterval(timer);
    };
  }, [initializing, accountId, backupKeyReady, autoBackup]);

  async function setAutoBackup(enabled: boolean) {
    setAutoBackupState(enabled);
    try {
      await save(SETTING_KEYS.cloudAutoBackup, String(enabled));
    } catch (error) {
      setAutoBackupState(!enabled);
      throw error;
    }
  }

  useEffect(() => {
    // Turned off, this PC never reads a saved session or reaches the server.
    if (cloudSetting === "false" || bootstrappedRef.current) {
      return;
    }
    // The ref makes this run once, also through React's development double
    // run, so the run that started finishes its work.
    bootstrappedRef.current = true;

    async function restoreStoredSession() {
      const { session: stored, keyringRead } = await loadPersistedSession();
      // A PC from before the switch keeps cloud backup while it is signed in.
      // Turning it off takes a keyring and settings that could be read, so a
      // locked keyring at login or a failed read decides nothing for good. A
      // development build reads a sign-in of its own, so it decides nothing.
      if (
        cloudSetting === undefined &&
        (stored !== null || (keyringRead && settingsRead)) &&
        !(await isDevelopmentBuild().catch(() => true))
      ) {
        await save(SETTING_KEYS.cloudBackup, String(stored !== null)).catch(
          logFailure("deciding whether this PC uses cloud backup"),
        );
      }
      if (!stored) {
        setBootstrapping(false);
        return;
      }

      applySession(stored);
      await syncBackupKeyState(stored);

      if (isExpired(stored.refresh_expires_at, 0)) {
        applyClearedSession();
        void forgetStoredSession();
        setBootstrapping(false);
        return;
      }

      const signOuts = signOutsRef.current;
      try {
        const refreshed = await refreshWithToken(stored);
        if (signOuts !== signOutsRef.current) {
          revokeLateSession(refreshed);
          return;
        }
        applySession(refreshed);
        await persistCloudSession(refreshed);
        await syncBackupKeyState(refreshed);
        void restoreDeviceRegistration(refreshed);
      } catch (error) {
        if (shouldClearPersistedSession(error)) {
          applyClearedSession();
          void forgetStoredSession();
        } else {
          setConnectionError(describeError(error));
          registerOnRenewalRef.current = true;
        }
      } finally {
        setBootstrapping(false);
      }
    }

    void restoreStoredSession();
  }, [cloudSetting, settingsRead, refreshWithToken, save]);

  function applySession(next: CloudAuthSession) {
    sessionRef.current = next;
    setSession(next);
    setConnectionError(null);
  }

  function applyClearedSession() {
    sessionRef.current = null;
    setSession(null);
    setDevice(null);
    setDeviceError(null);
    setConnectionError(null);
    setBackupKeyReady(false);
  }

  async function login(
    email: string,
    password: string,
    backupPassphrase?: string | null,
  ) {
    const next = await sessionFrom(DEFAULT_SERVER, "/v1/auth/login", { email, password });
    await persistCloudSession(next);
    if (backupPassphrase?.trim()) {
      const backups = await backupsForKeyCheck(() =>
        cloudGetJson<CloudBackupRecord[]>(next.server, "/v1/backups", next.access_token),
      );
      await persistBackupPassphrase(next, backupPassphrase, backups);
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
    backupPassphrase: string,
  ) {
    const next = await sessionFrom(DEFAULT_SERVER, "/v1/auth/signup", { email, password });
    await persistCloudSession(next);
    await persistBackupPassphrase(next, backupPassphrase, []);
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

    const signOuts = signOutsRef.current;
    const backups = await backupsForKeyCheck(() =>
      withAuthenticatedSession((active) => cloudGetJson<CloudBackupRecord[]>(active.server, "/v1/backups", active.access_token)),
    );
    await persistBackupPassphrase(current, normalized, backups);
    // Signed out while the check ran: the key must not stay behind.
    if (signOuts !== signOutsRef.current) {
      await clearCloudBackupKeySecure(current.user.id);
      throw new Error("You signed out of cloud backup.");
    }
    setBackupKeyReady(true);
  }

  /** Removes the backup key from this PC, so a different passphrase can be set. */
  async function forgetBackupPassphrase() {
    const current = sessionRef.current;
    if (!current) {
      return;
    }
    await clearCloudBackupKeySecure(current.user.id);
    setBackupKeyReady(false);
  }

  // The server signs out every other device and returns a new session for this one.
  async function changePassword(currentPassword: string, newPassword: string) {
    const next = await withAuthenticatedSession((current) =>
      sessionFrom(
        current.server,
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
      await cloudPostJson(current.server, "/v1/auth/logout", {
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
      const refreshed = await refreshWithToken(current);
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
      throw error;
    }
  }

  async function registerCurrentDevice() {
    const current = await ensureAuthenticatedSession();
    return registerDeviceForSession(current);
  }

  async function listBackups() {
    return withAuthenticatedSession((current) =>
      cloudGetJson<CloudBackupRecord[]>(current.server, "/v1/backups", current.access_token),
    );
  }

  async function listDevices() {
    return withAuthenticatedSession((current) => cloudGetJson<CloudDevice[]>(current.server, "/v1/devices", current.access_token));
  }

  async function removeDevice(deviceId: string) {
    await withAuthenticatedSession((current) =>
      cloudDelete(current.server, `/v1/devices/${encodeURIComponent(deviceId)}`, current.access_token),
    );
  }

  async function getStorage() {
    return withAuthenticatedSession((current) =>
      cloudGetJson<CloudStorage>(current.server, "/v1/storage", current.access_token),
    );
  }

  async function uploadRemoteBackup(label?: string | null) {
    return withAuthenticatedSession(async (current) => {
      await ensureBackupKeyForSession(current);
      const activeDeviceId = await ensureRemoteBackupDeviceId();
      return uploadRemoteBackupCommand(
        current.server,
        current.access_token,
        current.user.id,
        activeDeviceId,
        label ?? null,
      );
    });
  }

  async function restoreRemoteBackup(backupId: string, takeOver: boolean) {
    return withAuthenticatedSession(async (current) => {
      await ensureBackupKeyForSession(current);
      return restoreRemoteBackupCommand(
        current.server,
        current.access_token,
        current.user.id,
        backupId,
        takeOver,
      );
    });
  }

  async function listCloudPcs() {
    return withAuthenticatedSession(async (current) => {
      await ensureBackupKeyForSession(current);
      return listCloudPcsCommand(current.server, current.access_token, current.user.id);
    });
  }

  async function previewCloudMerge(backupId: string, trustNewKeys: boolean) {
    return withAuthenticatedSession(async (current) => {
      await ensureBackupKeyForSession(current);
      return previewCloudMergeCommand(current.server, current.access_token, current.user.id, backupId, trustNewKeys);
    });
  }

  async function mergeCloudBackup(backupId: string, choices: GameChoice[], trustNewKeys: boolean) {
    const summary = await withAuthenticatedSession(async (current) => {
      await ensureBackupKeyForSession(current);
      return mergeCloudBackupCommand(
        current.server,
        current.access_token,
        current.user.id,
        backupId,
        choices,
        trustNewKeys,
      );
    });
    setWaitingMerges((waiting) => waiting.filter((merge) => merge.device_id !== summary.device_id));
    return summary;
  }

  async function discardCloudMerge(backupId: string) {
    await discardCloudMergeCommand(backupId);
  }

  async function deleteBackup(backupId: string) {
    return withAuthenticatedSession((current) =>
      cloudDelete(
        current.server,
        `/v1/backups/${encodeURIComponent(backupId)}`,
        current.access_token,
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
      return refreshForAction(current);
    }

    return current;
  }

  /**
   * Refreshes the session for an action. A refresh that finishes after
   * signing out is revoked, and one the server refuses ends the session.
   */
  async function refreshForAction(current: CloudAuthSession): Promise<CloudAuthSession> {
    const signOuts = signOutsRef.current;
    let refreshed: CloudAuthSession;
    try {
      refreshed = await refreshWithToken(current);
    } catch (error) {
      if (shouldClearPersistedSession(error)) {
        applyClearedSession();
        void forgetStoredSession();
        throw new Error("Your cloud session expired. Sign in again.", {
          cause: error,
        });
      }
      throw error;
    }
    if (signOuts !== signOutsRef.current) {
      revokeLateSession(refreshed);
      throw new Error("You signed out of cloud backup.");
    }
    applySession(refreshed);
    await persistCloudSession(refreshed);
    return refreshed;
  }

  async function withAuthenticatedSession<T>(
    action: (activeSession: CloudAuthSession) => Promise<T>,
  ): Promise<T> {
    let current = await ensureAuthenticatedSession();

    try {
      return await action(current);
    } catch (error) {
      if (error instanceof CloudApiError && error.status === 401) {
        current = await refreshForAction(current);
        await syncBackupKeyState(current);
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
        activeSession.server,
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
    enabled,
    apiBaseUrl: session?.server ?? DEFAULT_SERVER,
    initializing,
    session,
    device,
    deviceError,
    connectionError,
    backupKeyReady,
    autoBackup,
    autoBackupError,
    setAutoBackup,
    login,
    signUp,
    setBackupPassphrase,
    forgetBackupPassphrase,
    changePassword,
    logout,
    refreshSession,
    registerCurrentDevice,
    listDevices,
    removeDevice,
    listBackups,
    getStorage,
    uploadRemoteBackup,
    restoreRemoteBackup,
    deleteBackup,
    listCloudPcs,
    previewCloudMerge,
    mergeCloudBackup,
    discardCloudMerge,
    waitingMerges,
  };

  return (
    <CloudSessionContext.Provider value={value}>
      {children}
    </CloudSessionContext.Provider>
  );
}
