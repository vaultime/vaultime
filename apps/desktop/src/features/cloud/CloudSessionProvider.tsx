// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import {
  createContext,
  useContext,
  useEffect,
  useEffectEvent,
  useRef,
  useState,
  type ReactNode,
} from "react";
import {
  CLOUD_API_BASE_URL,
  CloudApiError,
  cloudGetJson,
  cloudPostJson,
} from "@/lib/cloud-api";
import {
  getAppVersion,
  restoreRemoteBackup as restoreRemoteBackupCommand,
  uploadRemoteBackup as uploadRemoteBackupCommand,
} from "@/lib/tauri";
import type {
  CloudAdminInvite,
  CloudAuthSession,
  CloudBackupRecord,
  CloudBackupRestoreResult,
  CloudBackupUploadResult,
  CloudCreateAdminInviteInput,
  CloudDevice,
} from "@/lib/types";

const CLOUD_SESSION_STORAGE_KEY = "vaultime.cloud.session";
const CLOUD_DEVICE_ID_STORAGE_KEY = "vaultime.cloud.device-id";

interface CloudSessionContextValue {
  apiBaseUrl: string;
  initializing: boolean;
  session: CloudAuthSession | null;
  device: CloudDevice | null;
  deviceError: string | null;
  isAdmin: boolean;
  login: (email: string, password: string) => Promise<CloudAuthSession>;
  signUp: (
    email: string,
    password: string,
    inviteCode: string,
  ) => Promise<CloudAuthSession>;
  logout: () => Promise<void>;
  refreshSession: () => Promise<CloudAuthSession | null>;
  registerCurrentDevice: () => Promise<CloudDevice | null>;
  listBackups: () => Promise<CloudBackupRecord[]>;
  uploadRemoteBackup: (label?: string | null) => Promise<CloudBackupUploadResult>;
  restoreRemoteBackup: (backupId: string) => Promise<CloudBackupRestoreResult>;
  createAdminInvite: (
    input: CloudCreateAdminInviteInput,
  ) => Promise<CloudAdminInvite>;
}

const CloudSessionContext = createContext<CloudSessionContextValue | null>(null);

export function CloudSessionProvider({ children }: { children: ReactNode }) {
  const [initializing, setInitializing] = useState(true);
  const [session, setSession] = useState<CloudAuthSession | null>(null);
  const [device, setDevice] = useState<CloudDevice | null>(null);
  const [deviceError, setDeviceError] = useState<string | null>(null);
  const sessionRef = useRef<CloudAuthSession | null>(null);
  const bootstrappedRef = useRef(false);
  const appVersionRef = useRef<string | null>(null);
  const restoreDeviceRegistration = useEffectEvent(
    async (activeSession: CloudAuthSession) => {
      await tryRegisterDeviceForSession(activeSession);
    },
  );

  useEffect(() => {
    sessionRef.current = session;
  }, [session]);

  useEffect(() => {
    if (bootstrappedRef.current) {
      return;
    }
    bootstrappedRef.current = true;

    let cancelled = false;

    async function restoreStoredSession() {
      const stored = loadStoredSession();
      if (!stored) {
        if (!cancelled) {
          setInitializing(false);
        }
        return;
      }

      setPersistedSession(stored);

      if (isExpired(stored.refresh_expires_at, 0)) {
        clearPersistedSession();
        if (!cancelled) {
          setInitializing(false);
        }
        return;
      }

      try {
        const refreshed = await refreshWithToken(stored.refresh_token);
        if (cancelled) {
          return;
        }
        setPersistedSession(refreshed);
        void restoreDeviceRegistration(refreshed);
      } catch (error) {
        if (!cancelled) {
          clearPersistedSession();
          setDeviceError(String(error));
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
  }, []);

  function setPersistedSession(next: CloudAuthSession) {
    sessionRef.current = next;
    setSession(next);
    window.localStorage.setItem(CLOUD_SESSION_STORAGE_KEY, JSON.stringify(next));
  }

  function clearPersistedSession() {
    sessionRef.current = null;
    setSession(null);
    setDevice(null);
    setDeviceError(null);
    window.localStorage.removeItem(CLOUD_SESSION_STORAGE_KEY);
  }

  async function login(email: string, password: string) {
    const next = await cloudPostJson<CloudAuthSession>("/v1/auth/login", {
      email,
      password,
    });
    setPersistedSession(next);
    void tryRegisterDeviceForSession(next);
    return next;
  }

  async function signUp(email: string, password: string, inviteCode: string) {
    const next = await cloudPostJson<CloudAuthSession>("/v1/auth/signup", {
      email,
      password,
      invite_code: inviteCode,
    });
    setPersistedSession(next);
    void tryRegisterDeviceForSession(next);
    return next;
  }

  async function logout() {
    const current = sessionRef.current;
    clearPersistedSession();

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
      clearPersistedSession();
      return null;
    }

    try {
      const refreshed = await refreshWithToken(current.refresh_token);
      setPersistedSession(refreshed);
      return refreshed;
    } catch {
      clearPersistedSession();
      return null;
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

  async function uploadRemoteBackup(label?: string | null) {
    const activeDeviceId = await ensureRemoteBackupDeviceId();
    return withAuthenticatedSession((current) =>
      uploadRemoteBackupCommand(
        CLOUD_API_BASE_URL,
        current.access_token,
        activeDeviceId,
        label ?? null,
      ),
    );
  }

  async function restoreRemoteBackup(backupId: string) {
    return withAuthenticatedSession((current) =>
      restoreRemoteBackupCommand(
        CLOUD_API_BASE_URL,
        current.access_token,
        backupId,
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
      throw new Error("Sign in to use remote backup.");
    }

    if (isExpired(current.refresh_expires_at, 0)) {
      clearPersistedSession();
      throw new Error("Your cloud session expired. Sign in again.");
    }

    if (isExpired(current.expires_at, 60_000)) {
      const refreshed = await refreshWithToken(current.refresh_token);
      setPersistedSession(refreshed);
      return refreshed;
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
        setPersistedSession(refreshed);
        current = refreshed;
        return action(current);
      }

      throw error;
    }
  }

  async function refreshWithToken(refreshToken: string) {
    return cloudPostJson<CloudAuthSession>("/v1/auth/refresh", {
      refresh_token: refreshToken,
    });
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
      const message = String(error);
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
      appVersionRef.current = "0.1.0";
    }

    return appVersionRef.current;
  }

  const value: CloudSessionContextValue = {
    apiBaseUrl: CLOUD_API_BASE_URL,
    initializing,
    session,
    device,
    deviceError,
    isAdmin: session?.user.role === "admin",
    login,
    signUp,
    logout,
    refreshSession,
    registerCurrentDevice,
    listBackups,
    uploadRemoteBackup,
    restoreRemoteBackup,
    createAdminInvite,
  };

  return (
    <CloudSessionContext.Provider value={value}>
      {children}
    </CloudSessionContext.Provider>
  );
}

// eslint-disable-next-line react-refresh/only-export-components
export function useCloudSession() {
  const value = useContext(CloudSessionContext);
  if (!value) {
    throw new Error("useCloudSession must be used within CloudSessionProvider");
  }
  return value;
}

function loadStoredSession(): CloudAuthSession | null {
  const raw = window.localStorage.getItem(CLOUD_SESSION_STORAGE_KEY);
  if (!raw) {
    return null;
  }

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

function getOrCreateClientDeviceId() {
  const existing = window.localStorage.getItem(CLOUD_DEVICE_ID_STORAGE_KEY);
  if (existing) {
    return existing;
  }

  const created =
    typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `vaultime-${Math.random().toString(36).slice(2, 12)}`;
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
