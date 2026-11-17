// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { createContext, useContext } from "react";
import type {
  AutoMerge,
  CloudAuthSession,
  CloudBackupRecord,
  CloudBackupRestoreResult,
  CloudBackupUploadResult,
  CloudDevice,
  CloudPc,
  CloudServerInfo,
  CloudStorage,
  GameChoice,
  MergePreview,
  MergeSummary,
} from "@/lib/types";

export interface CloudSessionContextValue {
  /** Whether this PC uses cloud backup, which brings the Cloud page. Off on new installs. */
  enabled: boolean;
  /** The server to sign in to, checked, while this PC is signed out. */
  server: CloudServerInfo | null;
  /** Checks the server at `address` and remembers it for this PC. */
  connectServer: (address: string) => Promise<CloudServerInfo>;
  /** Lets the player pick another server. */
  forgetServer: () => void;
  /** Connects to the server of the last sign-in, once per run. A failure only goes to the log. */
  reconnectServer: () => void;
  initializing: boolean;
  session: CloudAuthSession | null;
  device: CloudDevice | null;
  deviceError: string | null;
  /** Why the saved session could not be renewed at start, until the next renewal works. */
  connectionError: string | null;
  backupKeyReady: boolean;
  /** Whether a signed in PC uploads a backup once a day. */
  autoBackup: boolean;
  /** Why the last automatic backup failed, until one succeeds. */
  autoBackupError: string | null;
  setAutoBackup: (enabled: boolean) => Promise<void>;
  login: (
    server: string,
    email: string,
    password: string,
    backupPassphrase?: string | null,
  ) => Promise<CloudAuthSession>;
  signUp: (
    server: string,
    email: string,
    password: string,
    backupPassphrase: string,
  ) => Promise<CloudAuthSession>;
  setBackupPassphrase: (passphrase: string) => Promise<void>;
  forgetBackupPassphrase: () => Promise<void>;
  changePassword: (currentPassword: string, newPassword: string) => Promise<void>;
  logout: () => Promise<void>;
  refreshSession: () => Promise<CloudAuthSession | null>;
  registerCurrentDevice: () => Promise<CloudDevice | null>;
  /** Every PC that signed in with the account. */
  listDevices: () => Promise<CloudDevice[]>;
  removeDevice: (deviceId: string) => Promise<void>;
  listBackups: () => Promise<CloudBackupRecord[]>;
  getStorage: () => Promise<CloudStorage>;
  uploadRemoteBackup: (label?: string | null) => Promise<CloudBackupUploadResult>;
  /** With `takeOver`, a fresh install continues as the PC that made the backup. */
  restoreRemoteBackup: (backupId: string, takeOver: boolean) => Promise<CloudBackupRestoreResult>;
  deleteBackup: (backupId: string) => Promise<void>;
  /** The newest cloud backup of every other PC of the account. */
  listCloudPcs: () => Promise<CloudPc[]>;
  previewCloudMerge: (backupId: string, trustNewKeys: boolean) => Promise<MergePreview>;
  mergeCloudBackup: (backupId: string, choices: GameChoice[], trustNewKeys: boolean) => Promise<MergeSummary>;
  discardCloudMerge: (backupId: string) => Promise<void>;
  /** Newer backups of PCs merged before that did not come in by themselves, as they need the player. */
  waitingMerges: AutoMerge[];
}

export const CloudSessionContext = createContext<CloudSessionContextValue | null>(null);

export function useCloudSession(): CloudSessionContextValue {
  const value = useContext(CloudSessionContext);
  if (!value) {
    throw new Error("useCloudSession must be used within CloudSessionProvider");
  }
  return value;
}
