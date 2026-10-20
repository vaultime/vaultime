// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useEffectEvent, useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { HardDriveUpload, KeyRound, Loader2, LogOut, RefreshCw, RotateCcw } from "lucide-react";
import { Notice, PageHeader, PageRow, PageSection } from "@/components/layout/Page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { SignInForm, SignUpForm, UnlockBackupsForm } from "@/features/cloud/AccountForms";
import { BetaApplications } from "@/features/cloud/BetaApplications";
import { ChangePasswordDialog } from "@/features/cloud/ChangePasswordDialog";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { useLibrary } from "@/features/library/library-context";
import { useFinishRestore } from "@/features/settings/use-finish-restore";
import type { CloudBackupRecord, CloudStorage } from "@/lib/types";
import { getThisPc } from "@/lib/tauri";
import { describeError } from "@/lib/utils";
import { capitalize } from "@/lib/words";
import { BackupRow, DeleteBackupDialog, RestoreBackupDialog } from "./BackupDialogs";
import {
  backupSource,
  formatApiHostname,
  formatByteSize,
  formatTimestamp,
  loadCachedBackups,
  persistCachedBackups,
} from "./backups";
import { CreateInviteDialog } from "./CreateInviteDialog";
import { RegisteredPcsSection } from "./RegisteredPcsSection";

export function CloudPage() {
  const {
    apiBaseUrl,
    autoBackup,
    autoBackupError,
    backupKeyReady,
    deleteBackup,
    connectionError,
    device,
    deviceError,
    forgetBackupPassphrase,
    initializing,
    isAdmin,
    listBackups,
    getStorage,
    logout,
    refreshSession,
    registerCurrentDevice,
    restoreRemoteBackup,
    session,
    setAutoBackup,
    uploadRemoteBackup,
  } = useCloudSession();
  const { thisDeviceId: deviceId } = useLibrary();
  const finishRestore = useFinishRestore();
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [authBusy, setAuthBusy] = useState(false);
  const [deviceBusy, setDeviceBusy] = useState(false);
  const [backupsLoading, setBackupsLoading] = useState(false);
  const [storage, setStorage] = useState<CloudStorage | null>(null);
  const [remoteBackupBusy, setRemoteBackupBusy] = useState(false);
  const [inviteDialogOpen, setInviteDialogOpen] = useState(false);
  const [passwordDialogOpen, setPasswordDialogOpen] = useState(false);
  const [restoreDialogOpen, setRestoreDialogOpen] = useState(false);
  const [remoteBackups, setRemoteBackups] = useState<CloudBackupRecord[]>([]);
  const [restoreTarget, setRestoreTarget] = useState<CloudBackupRecord | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<CloudBackupRecord | null>(null);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [restartRequired, setRestartRequired] = useState(false);
  // A PC without history of its own continues as the PC of the backup it
  // restores. Null while the restore dialog still asks the core.
  const [freshInstall, setFreshInstall] = useState<boolean | null>(false);
  const [takeOver, setTakeOver] = useState(true);
  // The core takes over only what the dialog offered, so a check that failed
  // or a backup without its PC never takes over unasked.
  const takesOver =
    freshInstall === true &&
    deviceId !== null &&
    restoreTarget !== null &&
    backupSource(restoreTarget) !== null &&
    backupSource(restoreTarget) !== deviceId;

  function requestBackups(accountId: string) {
    getStorage()
      .then(setStorage)
      .catch(() => setStorage(null));
    return listBackups()
      .then((backups) => {
        setRemoteBackups(backups);
        persistCachedBackups(accountId, backups);
      })
      .catch((error: unknown) => {
        const cached = loadCachedBackups(accountId);
        if (cached) {
          setRemoteBackups(cached);
          setStatusMessage(
            "The cloud server is not reachable. Showing the cloud backups it listed last time.",
          );
        } else {
          setErrorMessage(describeError(error));
        }
      })
      .finally(() => setBackupsLoading(false));
  }

  async function fetchBackups(accountId: string) {
    setBackupsLoading(true);
    await requestBackups(accountId);
  }

  const loadBackups = useEffectEvent((accountId: string) => {
    void requestBackups(accountId);
  });

  // Reset during render when the account changes so one account never shows another's backups.
  const accountId = session?.user.id ?? null;
  const [backupsAccountId, setBackupsAccountId] = useState<string | null>(null);
  if (backupsAccountId !== accountId) {
    setBackupsAccountId(accountId);
    setRemoteBackups([]);
    setStorage(null);
    setBackupsLoading(accountId !== null);
  }

  useEffect(() => {
    if (accountId) {
      loadBackups(accountId);
    }
  }, [accountId]);

  function showDone(message: string) {
    setErrorMessage(null);
    setStatusMessage(message);
  }

  async function handleRefreshSession() {
    setAuthBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const refreshed = await refreshSession();
      if (!refreshed) {
        setStatusMessage("Cloud session expired. Sign in again.");
        return;
      }
      setStatusMessage("Cloud session refreshed.");
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setAuthBusy(false);
    }
  }

  async function handleRegisterDevice() {
    setDeviceBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const registered = await registerCurrentDevice();
      if (registered) {
        setStatusMessage(`This PC is registered as ${registered.device_name}.`);
      }
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setDeviceBusy(false);
    }
  }

  async function handleLogout() {
    setAuthBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      await logout();
      setStatusMessage("Signed out of cloud backup.");
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setAuthBusy(false);
    }
  }

  async function handleReloadBackups() {
    if (!session) {
      return;
    }

    setErrorMessage(null);
    await fetchBackups(session.user.id);
  }

  async function handleCreateRemoteBackup() {
    setRemoteBackupBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const result = await uploadRemoteBackup();
      setRemoteBackups((current) => {
        const next = [
          result.backup,
          ...current.filter((backup) => backup.id !== result.backup.id),
        ];
        if (session) {
          persistCachedBackups(session.user.id, next);
        }
        return next;
      });
      setStatusMessage(
        `Cloud backup uploaded with ${result.payload_summary.games_count} games and ${result.payload_summary.sessions_count} sessions.`,
      );
    } catch (error) {
      setErrorMessage(describeError(error));
      return;
    } finally {
      setRemoteBackupBusy(false);
    }

    // Past the backup limit or the space, the server removed the oldest ones.
    if (session) {
      await fetchBackups(session.user.id);
    }
  }

  async function handleRestoreRemoteBackup() {
    if (!restoreTarget) {
      return;
    }

    setRemoteBackupBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const result = await restoreRemoteBackup(restoreTarget.id, takeOver && takesOver);
      setRestoreDialogOpen(false);
      setRestoreTarget(null);
      if (result.restored_summary.restart_required) {
        setRestartRequired(true);
      }
      setStatusMessage(await finishRestore("Cloud backup", result.restored_summary));
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setRemoteBackupBusy(false);
    }
  }

  async function handleDeleteRemoteBackup() {
    if (!deleteTarget || !session) {
      return;
    }

    const target = deleteTarget;
    setRemoteBackupBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      await deleteBackup(target.id);
      setRemoteBackups((current) => {
        const next = current.filter((backup) => backup.id !== target.id);
        persistCachedBackups(session.user.id, next);
        return next;
      });
      setStatusMessage("Cloud backup deleted.");
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setDeleteDialogOpen(false);
      setRemoteBackupBusy(false);
    }

    await fetchBackups(session.user.id);
  }

  async function handleRestart() {
    try {
      await relaunch();
    } catch (error) {
      setErrorMessage(describeError(error));
    }
  }

  function openRestoreDialog(backup: CloudBackupRecord) {
    setRestoreTarget(backup);
    setRestoreDialogOpen(true);
    setFreshInstall(null);
    setTakeOver(true);
    getThisPc()
      .then((pc) => setFreshInstall(pc.fresh))
      .catch(() => setFreshInstall(false));
  }

  function openDeleteDialog(backup: CloudBackupRecord) {
    setDeleteTarget(backup);
    setDeleteDialogOpen(true);
  }

  return (
    <div className="pb-16">
      <PageHeader
        overline={session ? `Signed in as ${session.user.email}` : "Invite only"}
        title="Cloud"
        aside={isAdmin ? <Badge variant="amber">Admin</Badge> : undefined}
      >
        {initializing
          ? "Looking for a saved session."
          : session
            ? backupKeyReady
              ? "Backups are encrypted on this PC before they leave it. The server never sees your passphrase."
              : "Unlock backups on this PC with your backup passphrase to start."
            : "Encrypted backups of your library on a small server, for invited accounts. Vaultime works fully without it."}
      </PageHeader>

      <div className="px-8 xl:px-14">
        {errorMessage && (
          <Notice tone="warning" className="mt-6">
            {errorMessage}
          </Notice>
        )}
        {statusMessage && <Notice className="mt-6">{statusMessage}</Notice>}
        {connectionError && !errorMessage && (
          <Notice tone="warning" className="mt-6">
            {connectionError} Vaultime tries again with the next cloud action.
          </Notice>
        )}
        {restartRequired && (
          <Notice className="mt-6">
            Restart Vaultime to continue tracking with the restored history.
            <Button size="sm" onClick={() => void handleRestart()}>
              <RotateCcw className="size-3.5" />
              Restart now
            </Button>
          </Notice>
        )}

        {!session ? (
          <>
            <PageSection title="Sign in" description="With an account you already have.">
              <SignInForm onDone={showDone} />
            </PageSection>

            <PageSection
              title="Create an account"
              description="You need an invite code. The backup passphrase encrypts your backups and never leaves this PC, so keep it somewhere safe."
            >
              <SignUpForm onDone={showDone} />
            </PageSection>
          </>
        ) : (
          <>
            <PageSection
              title="Backups"
              description="Kept on the server for this account. Past the backup limit, or when the space is full, a new backup replaces the oldest ones."
            >
              {!backupKeyReady && <UnlockBackupsForm onDone={showDone} />}

              <PageRow
                label="Back up every day"
                htmlFor="cloud-auto-backup"
                hint="While this PC is signed in and its backups are unlocked."
              >
                <Switch
                  id="cloud-auto-backup"
                  checked={autoBackup}
                  onCheckedChange={(checked) =>
                    setAutoBackup(checked).catch((error: unknown) => setErrorMessage(describeError(error)))
                  }
                />
              </PageRow>
              {autoBackup && autoBackupError && (
                <Notice tone="warning" className="my-4">
                  The last daily backup failed: {autoBackupError}
                </Notice>
              )}

              {storage && (
                <PageRow
                  label="Space used"
                  hint="Artwork is stored once for all backups, so a new backup adds little more than your history."
                >
                  <span className="font-mono text-sm">
                    {formatByteSize(storage.backup_bytes + storage.artwork_bytes)} of{" "}
                    {formatByteSize(storage.limit_bytes)}
                  </span>
                </PageRow>
              )}

              <div className="mt-6 flex flex-wrap gap-3">
                <Button onClick={() => void handleCreateRemoteBackup()} disabled={remoteBackupBusy || !backupKeyReady}>
                  {remoteBackupBusy ? <Loader2 className="size-4 animate-spin" /> : <HardDriveUpload className="size-4" />}
                  Back up now
                </Button>
                <Button
                  variant="outline"
                  onClick={() => void handleReloadBackups()}
                  disabled={backupsLoading || remoteBackupBusy}
                >
                  <RefreshCw className="size-4" />
                  Reload
                </Button>
              </div>

              <div className="mt-6">
                {backupsLoading ? (
                  <p className="flex items-center gap-2 py-3 text-sm text-faint">
                    <Loader2 className="size-4 animate-spin" />
                    Loading backups
                  </p>
                ) : remoteBackups.length === 0 ? (
                  <p className="py-3 text-sm text-faint">No backups on the server yet.</p>
                ) : (
                  remoteBackups.map((backup) => (
                    <BackupRow
                      key={backup.id}
                      backup={backup}
                      thisDeviceId={deviceId}
                      canRestore={backupKeyReady}
                      busy={remoteBackupBusy}
                      onRestore={() => openRestoreDialog(backup)}
                      onDelete={() => openDeleteDialog(backup)}
                    />
                  ))
                )}
              </div>
            </PageSection>

            <PageSection title="This PC" description="The server links every backup to the PC that made it.">
              {device ? (
                <>
                  <PageRow label="Name">{device.device_name}</PageRow>
                  <PageRow label="System">{capitalize(device.platform)}</PageRow>
                  <PageRow label="Last seen">{formatTimestamp(device.last_seen_at)}</PageRow>
                  <PageRow label="Device id">
                    <span className="font-mono text-xs text-faint">{device.client_device_id}</span>
                  </PageRow>
                </>
              ) : (
                <p className="text-sm text-faint">This PC is not registered yet.</p>
              )}
              <PageRow
                label="Backup passphrase"
                hint={
                  backupKeyReady
                    ? "Change it only if these backups were made with another passphrase."
                    : "Enter it above to unlock the backups."
                }
              >
                <span className="flex items-center gap-3">
                  <span className={backupKeyReady ? "text-soft" : "text-amber"}>
                    {backupKeyReady ? "Unlocked, kept in the system keychain" : "Locked"}
                  </span>
                  {backupKeyReady && (
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() =>
                        forgetBackupPassphrase().catch((error: unknown) => setErrorMessage(describeError(error)))
                      }
                    >
                      Change
                    </Button>
                  )}
                </span>
              </PageRow>
              {deviceError && (
                <Notice tone="warning" className="mt-4">
                  Registering this PC failed: {deviceError}
                </Notice>
              )}
              <Button variant="outline" size="sm" className="mt-4" onClick={handleRegisterDevice} disabled={deviceBusy}>
                {deviceBusy ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
                Register again
              </Button>
            </PageSection>

            <RegisteredPcsSection onError={setErrorMessage} />

            <PageSection title="Account">
              <PageRow label="Email">{session.user.email}</PageRow>
              <PageRow label="Role">{capitalize(session.user.role)}</PageRow>
              <PageRow label="Signed in until">{formatTimestamp(session.refresh_expires_at)}</PageRow>
              <div className="mt-4 flex flex-wrap gap-2">
                <Button variant="outline" size="sm" onClick={handleRefreshSession} disabled={authBusy}>
                  {authBusy ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
                  Renew session
                </Button>
                <Button variant="outline" size="sm" onClick={() => setPasswordDialogOpen(true)} disabled={authBusy}>
                  <KeyRound className="size-3.5" />
                  Change password
                </Button>
                <Button variant="ghost" size="sm" onClick={handleLogout} disabled={authBusy}>
                  <LogOut className="size-3.5" />
                  Sign out
                </Button>
              </div>
            </PageSection>

            {isAdmin && (
              <PageSection
                title="Invites"
                description="Only admins see this. A new code is shown once. The server keeps only a hash of it."
              >
                <Button onClick={() => setInviteDialogOpen(true)}>
                  <KeyRound className="size-4" />
                  Create an invite
                </Button>
              </PageSection>
            )}
            {isAdmin && <BetaApplications onError={setErrorMessage} />}
          </>
        )}

        <PageSection title="Server">
          <PageRow label="Address">
            <span className="font-mono text-sm">{formatApiHostname(apiBaseUrl)}</span>
          </PageRow>
          <PageRow label="Encryption" hint="With your backup passphrase, before the backup leaves this PC.">
            XChaCha20-Poly1305
          </PageRow>
          <PageRow label="Checksums" hint="The server checks every upload before it accepts it.">
            SHA-256
          </PageRow>
        </PageSection>
      </div>

      <ChangePasswordDialog
        open={passwordDialogOpen}
        onOpenChange={setPasswordDialogOpen}
        onChanged={() => {
          setErrorMessage(null);
          setStatusMessage("Password changed. Your other PCs are signed out.");
        }}
      />

      <CreateInviteDialog
        open={inviteDialogOpen}
        onOpenChange={setInviteDialogOpen}
        onStatus={setStatusMessage}
        onError={setErrorMessage}
      />

      <RestoreBackupDialog
        open={restoreDialogOpen}
        onOpenChange={(open) => {
          setRestoreDialogOpen(open);
          if (!open) {
            setRestoreTarget(null);
          }
        }}
        target={restoreTarget}
        takesOver={takesOver}
        ready={freshInstall !== null}
        takeOver={takeOver}
        onTakeOverChange={setTakeOver}
        busy={remoteBackupBusy}
        onRestore={() => void handleRestoreRemoteBackup()}
        onCancel={() => setRestoreDialogOpen(false)}
      />

      <DeleteBackupDialog
        open={deleteDialogOpen}
        onOpenChange={setDeleteDialogOpen}
        target={deleteTarget}
        busy={remoteBackupBusy}
        onDelete={() => void handleDeleteRemoteBackup()}
      />
    </div>
  );
}
