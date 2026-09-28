// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { createContext, useContext } from "react";
import type {
  CloudAdminInvite,
  CloudAuthSession,
  CloudBackupRecord,
  CloudBackupRestoreResult,
  CloudBackupUploadResult,
  CloudCreateAdminInviteInput,
  CloudDevice,
} from "@/lib/types";

export interface CloudSessionContextValue {
  apiBaseUrl: string;
  initializing: boolean;
  session: CloudAuthSession | null;
  device: CloudDevice | null;
  deviceError: string | null;
  backupKeyReady: boolean;
  /** Whether a signed in PC uploads a backup once a day. */
  autoBackup: boolean;
  setAutoBackup: (enabled: boolean) => Promise<void>;
  isAdmin: boolean;
  login: (
    email: string,
    password: string,
    backupPassphrase?: string | null,
  ) => Promise<CloudAuthSession>;
  signUp: (
    email: string,
    password: string,
    inviteCode: string,
    backupPassphrase: string,
  ) => Promise<CloudAuthSession>;
  setBackupPassphrase: (passphrase: string) => Promise<void>;
  changePassword: (currentPassword: string, newPassword: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshSession: () => Promise<CloudAuthSession | null>;
  registerCurrentDevice: () => Promise<CloudDevice | null>;
  listBackups: () => Promise<CloudBackupRecord[]>;
  uploadRemoteBackup: (label?: string | null) => Promise<CloudBackupUploadResult>;
  restoreRemoteBackup: (backupId: string) => Promise<CloudBackupRestoreResult>;
  deleteBackup: (backupId: string) => Promise<void>;
  createAdminInvite: (
    input: CloudCreateAdminInviteInput,
  ) => Promise<CloudAdminInvite>;
}

export const CloudSessionContext = createContext<CloudSessionContextValue | null>(null);

export function useCloudSession(): CloudSessionContextValue {
  const value = useContext(CloudSessionContext);
  if (!value) {
    throw new Error("useCloudSession must be used within CloudSessionProvider");
  }
  return value;
}
