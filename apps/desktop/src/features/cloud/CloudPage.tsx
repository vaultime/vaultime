// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useEffectEvent, useState, type FormEvent } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import {
  ArchiveRestore,
  Copy,
  HardDriveUpload,
  KeyRound,
  Loader2,
  LogOut,
  RefreshCw,
  RotateCcw,
  Trash2,
} from "lucide-react";
import { Notice, PageHeader, PageRow, PageSection } from "@/components/layout/Page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Switch } from "@/components/ui/switch";
import { BetaApplications } from "@/features/cloud/BetaApplications";
import { ChangePasswordDialog } from "@/features/cloud/ChangePasswordDialog";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { Field } from "@/components/ui/field";
import { BACKUP_PASSPHRASE_TOO_SHORT } from "@/lib/cloud-api";
import {
  BYTES_PER_KIB,
  CLOUD_BACKUPS_CACHE_KEY_PREFIX,
  INVITE_CODE_PREFIX,
  MIN_BACKUP_PASSPHRASE_CHARS,
  SIZE_ONE_DECIMAL_BELOW,
} from "@/lib/constants";
import { formatLongDate, formatSessionStart } from "@/lib/time";
import type { CloudAdminInvite, CloudBackupRecord, CloudStorage } from "@/lib/types";
import { getDeviceId } from "@/lib/tauri";
import { describeError } from "@/lib/utils";
import { capitalize, plural, whichPc } from "@/lib/words";

function formatTimestamp(value: string | null | undefined) {
  if (!value) {
    return "Not set";
  }
  try {
    return formatLongDate(value);
  } catch {
    return value;
  }
}

/** What a backup restores. Newer backups keep their artwork apart, stored once for all of them. */
function backupSize(backup: CloudBackupRecord) {
  return formatByteSize(backup.size_bytes + (backup.metadata_json?.artwork_bytes ?? 0));
}

function formatByteSize(bytes: number) {
  if (bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KiB", "MiB", "GiB", "TiB"];
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(BYTES_PER_KIB)),
    units.length - 1,
  );
  const value = bytes / BYTES_PER_KIB ** exponent;
  return `${value.toFixed(value >= SIZE_ONE_DECIMAL_BELOW || exponent === 0 ? 0 : 1)} ${units[exponent]}`;
}

function toIsoTimestamp(value: string) {
  if (!value) {
    return null;
  }

  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return null;
  }

  return parsed.toISOString();
}

function formatApiHostname(value: string) {
  try {
    return new URL(value).hostname;
  } catch {
    return value.replace(/^https?:\/\//, "");
  }
}

function remoteBackupCacheKey(accountId: string) {
  return `${CLOUD_BACKUPS_CACHE_KEY_PREFIX}${accountId}`;
}

function loadCachedRemoteBackups(accountId: string): CloudBackupRecord[] | null {
  try {
    const raw = window.localStorage.getItem(remoteBackupCacheKey(accountId));
    if (!raw) {
      return null;
    }

    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? (parsed as CloudBackupRecord[]) : null;
  } catch {
    return null;
  }
}

function persistCachedRemoteBackups(
  accountId: string,
  backups: CloudBackupRecord[],
) {
  window.localStorage.setItem(
    remoteBackupCacheKey(accountId),
    JSON.stringify(backups),
  );
}

export function CloudPage() {
  const {
    apiBaseUrl,
    autoBackup,
    autoBackupError,
    backupKeyReady,
    createAdminInvite,
    deleteBackup,
    device,
    deviceError,
    forgetBackupPassphrase,
    initializing,
    isAdmin,
    listBackups,
    getStorage,
    login,
    logout,
    refreshSession,
    registerCurrentDevice,
    restoreRemoteBackup,
    setBackupPassphrase,
    session,
    setAutoBackup,
    signUp,
    uploadRemoteBackup,
  } = useCloudSession();
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
  const [inviteBusy, setInviteBusy] = useState(false);
  const [backupKeyBusy, setBackupKeyBusy] = useState(false);
  const [lastInvite, setLastInvite] = useState<CloudAdminInvite | null>(null);
  const [copiedInvite, setCopiedInvite] = useState(false);
  const [remoteBackups, setRemoteBackups] = useState<CloudBackupRecord[]>([]);
  const [restoreTarget, setRestoreTarget] = useState<CloudBackupRecord | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<CloudBackupRecord | null>(null);
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);
  const [restartRequired, setRestartRequired] = useState(false);
  const [loginEmail, setLoginEmail] = useState("");
  const [loginPassword, setLoginPassword] = useState("");
  const [signUpEmail, setSignUpEmail] = useState("");
  const [signUpPassword, setSignUpPassword] = useState("");
  const [signUpBackupPassphrase, setSignUpBackupPassphrase] = useState("");
  const [signUpBackupPassphraseConfirm, setSignUpBackupPassphraseConfirm] =
    useState("");
  const [deviceBackupPassphrase, setDeviceBackupPassphrase] = useState("");
  const [deviceBackupPassphraseConfirm, setDeviceBackupPassphraseConfirm] =
    useState("");
  const [inviteCode, setInviteCode] = useState("");
  const [invitePrefix, setInvitePrefix] = useState(INVITE_CODE_PREFIX);
  const [inviteMaxRedemptions, setInviteMaxRedemptions] = useState("1");
  const [inviteExpiry, setInviteExpiry] = useState("");
  const [inviteNote, setInviteNote] = useState("");
  const [deviceId, setDeviceId] = useState<string | null>(null);

  useEffect(() => {
    getDeviceId()
      .then(setDeviceId)
      .catch(() => {});
  }, []);

  function requestBackups(accountId: string) {
    getStorage()
      .then(setStorage)
      .catch(() => setStorage(null));
    return listBackups()
      .then((backups) => {
        setRemoteBackups(backups);
        persistCachedRemoteBackups(accountId, backups);
      })
      .catch((error: unknown) => {
        const cached = loadCachedRemoteBackups(accountId);
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

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAuthBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const nextSession = await login(loginEmail, loginPassword);
      setStatusMessage(`Signed in as ${nextSession.user.email}.`);
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setAuthBusy(false);
    }
  }

  async function handleSignUp(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAuthBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      if (signUpBackupPassphrase.trim().length < MIN_BACKUP_PASSPHRASE_CHARS) {
        throw new Error(BACKUP_PASSPHRASE_TOO_SHORT);
      }
      if (signUpBackupPassphrase !== signUpBackupPassphraseConfirm) {
        throw new Error("Backup passphrase confirmation does not match.");
      }
      const nextSession = await signUp(
        signUpEmail,
        signUpPassword,
        inviteCode,
        signUpBackupPassphrase,
      );
      setStatusMessage(`Cloud account created for ${nextSession.user.email}.`);
      setInviteCode("");
      setSignUpBackupPassphrase("");
      setSignUpBackupPassphraseConfirm("");
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setAuthBusy(false);
    }
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

  async function handleSetBackupPassphrase(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setBackupKeyBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      if (deviceBackupPassphrase.trim().length < MIN_BACKUP_PASSPHRASE_CHARS) {
        throw new Error(BACKUP_PASSPHRASE_TOO_SHORT);
      }
      if (deviceBackupPassphrase !== deviceBackupPassphraseConfirm) {
        throw new Error("Backup passphrase confirmation does not match.");
      }

      await setBackupPassphrase(deviceBackupPassphrase);
      setDeviceBackupPassphrase("");
      setDeviceBackupPassphraseConfirm("");
      setStatusMessage("Backup passphrase unlocked for this PC.");
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setBackupKeyBusy(false);
    }
  }

  async function handleGenerateInvite(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setInviteBusy(true);
    setCopiedInvite(false);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const maxRedemptions = Number.parseInt(inviteMaxRedemptions, 10);
      if (!Number.isFinite(maxRedemptions) || maxRedemptions < 1) {
        throw new Error("Max redemptions must be at least 1.");
      }

      const generated = await createAdminInvite({
        prefix: invitePrefix.trim() || null,
        max_redemptions: maxRedemptions,
        expires_at: toIsoTimestamp(inviteExpiry),
        note: inviteNote.trim() || null,
      });

      setLastInvite(generated);
      setStatusMessage("New invite key generated.");
    } catch (error) {
      setErrorMessage(describeError(error));
    } finally {
      setInviteBusy(false);
    }
  }

  async function handleCopyInvite() {
    if (!lastInvite?.code) {
      return;
    }

    try {
      await navigator.clipboard.writeText(lastInvite.code);
      setCopiedInvite(true);
      setStatusMessage("Invite code copied.");
    } catch (error) {
      setErrorMessage(describeError(error));
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
          persistCachedRemoteBackups(session.user.id, next);
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

    // Past the backup limit the server removed the oldest one.
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
      const result = await restoreRemoteBackup(restoreTarget.id);
      setRestoreDialogOpen(false);
      setRestoreTarget(null);
      setStatusMessage("Cloud backup restored.");
      if (result.restored_summary.restart_required) {
        setRestartRequired(true);
      }
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
        persistCachedRemoteBackups(session.user.id, next);
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
              <form className="grid max-w-[520px] gap-4" onSubmit={handleLogin}>
                <Field id="cloud-login-email" label="Email">
                  <Input
                    id="cloud-login-email"
                    autoComplete="email"
                    value={loginEmail}
                    onChange={(event) => setLoginEmail(event.target.value)}
                  />
                </Field>
                <Field id="cloud-login-password" label="Password">
                  <Input
                    id="cloud-login-password"
                    type="password"
                    autoComplete="current-password"
                    value={loginPassword}
                    onChange={(event) => setLoginPassword(event.target.value)}
                  />
                </Field>
                <div>
                  <Button type="submit" disabled={authBusy || !loginEmail.trim() || !loginPassword}>
                    {authBusy && <Loader2 className="size-4 animate-spin" />}
                    {authBusy ? "Signing in" : "Sign in"}
                  </Button>
                </div>
              </form>
            </PageSection>

            <PageSection
              title="Create an account"
              description="You need an invite code. The backup passphrase encrypts your backups and never leaves this PC, so keep it somewhere safe."
            >
              <form className="grid max-w-[520px] gap-4" onSubmit={handleSignUp}>
                <Field id="cloud-signup-email" label="Email">
                  <Input
                    id="cloud-signup-email"
                    autoComplete="email"
                    value={signUpEmail}
                    onChange={(event) => setSignUpEmail(event.target.value)}
                  />
                </Field>
                <Field id="cloud-signup-password" label="Password">
                  <Input
                    id="cloud-signup-password"
                    type="password"
                    autoComplete="new-password"
                    value={signUpPassword}
                    onChange={(event) => setSignUpPassword(event.target.value)}
                  />
                </Field>
                <Field id="cloud-signup-invite" label="Invite code">
                  <Input
                    id="cloud-signup-invite"
                    // Six groups of four, INVITE_BODY_CHARS in the API's constants.rs.
                    placeholder={`${INVITE_CODE_PREFIX}-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX`}
                    value={inviteCode}
                    onChange={(event) => setInviteCode(event.target.value)}
                    className="font-mono"
                  />
                </Field>
                <Field id="cloud-signup-backup-passphrase" label="Backup passphrase">
                  <Input
                    id="cloud-signup-backup-passphrase"
                    type="password"
                    autoComplete="new-password"
                    value={signUpBackupPassphrase}
                    onChange={(event) => setSignUpBackupPassphrase(event.target.value)}
                  />
                </Field>
                <Field id="cloud-signup-backup-passphrase-confirm" label="Backup passphrase again">
                  <Input
                    id="cloud-signup-backup-passphrase-confirm"
                    type="password"
                    autoComplete="new-password"
                    value={signUpBackupPassphraseConfirm}
                    onChange={(event) => setSignUpBackupPassphraseConfirm(event.target.value)}
                  />
                </Field>
                <div>
                  <Button
                    type="submit"
                    disabled={
                      authBusy ||
                      !signUpEmail.trim() ||
                      !signUpPassword ||
                      !inviteCode.trim() ||
                      !signUpBackupPassphrase ||
                      !signUpBackupPassphraseConfirm
                    }
                  >
                    {authBusy && <Loader2 className="size-4 animate-spin" />}
                    {authBusy ? "Creating the account" : "Create account"}
                  </Button>
                </div>
              </form>
            </PageSection>
          </>
        ) : (
          <>
            <PageSection
              title="Backups"
              description="Kept on the server for this account. Past the backup limit, a new backup replaces the oldest. When the space is full, delete older backups to make room."
            >
              {!backupKeyReady && (
                <form className="mb-8 grid max-w-[520px] gap-4" onSubmit={handleSetBackupPassphrase}>
                  <p className="text-sm leading-relaxed text-soft">
                    New account: choose a backup passphrase now. Existing backups: enter the passphrase you used for
                    them.
                  </p>
                  <Field id="device-backup-passphrase" label="Backup passphrase">
                    <Input
                      id="device-backup-passphrase"
                      type="password"
                      autoComplete="new-password"
                      value={deviceBackupPassphrase}
                      onChange={(event) => setDeviceBackupPassphrase(event.target.value)}
                    />
                  </Field>
                  <Field id="device-backup-passphrase-confirm" label="Backup passphrase again">
                    <Input
                      id="device-backup-passphrase-confirm"
                      type="password"
                      autoComplete="new-password"
                      value={deviceBackupPassphraseConfirm}
                      onChange={(event) => setDeviceBackupPassphraseConfirm(event.target.value)}
                    />
                  </Field>
                  <div>
                    <Button
                      type="submit"
                      disabled={backupKeyBusy || !deviceBackupPassphrase || !deviceBackupPassphraseConfirm}
                    >
                      {backupKeyBusy && <Loader2 className="size-4 animate-spin" />}
                      {backupKeyBusy ? "Unlocking" : "Unlock backups on this PC"}
                    </Button>
                  </div>
                </form>
              )}

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
                    <div
                      key={backup.id}
                      className="flex flex-wrap items-baseline gap-x-5 gap-y-2 border-b border-rule py-3.5 last:border-b-0"
                    >
                      <span className="w-[20ch] shrink-0 font-mono text-[13px] text-faint">
                        {formatSessionStart(backup.uploaded_at)}
                      </span>
                      <div className="min-w-0 flex-1">
                        <div className="text-[15px]">{backup.label ?? "Backup"}</div>
                        <div className="mt-0.5 text-[13px] text-faint">
                          {[
                            backup.metadata_json && plural(backup.metadata_json.games_count, "game"),
                            backup.metadata_json && plural(backup.metadata_json.sessions_count, "session"),
                            backupSize(backup),
                            whichPc(backup.metadata_json?.source_device_id ?? backup.client_device_id, deviceId) &&
                              `made on ${whichPc(backup.metadata_json?.source_device_id ?? backup.client_device_id, deviceId)}`,
                          ]
                            .filter(Boolean)
                            .join(", ")}
                        </div>
                      </div>
                      {backup.status !== "complete" && <Badge variant="amber">{backup.status}</Badge>}
                      <div className="flex gap-1">
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => openRestoreDialog(backup)}
                          disabled={remoteBackupBusy || backup.status !== "complete" || !backupKeyReady}
                        >
                          <ArchiveRestore className="size-3.5" />
                          Restore
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          aria-label={`Delete ${backup.label ?? "backup"}`}
                          onClick={() => openDeleteDialog(backup)}
                          disabled={remoteBackupBusy}
                        >
                          <Trash2 className="size-3.5" />
                        </Button>
                      </div>
                    </div>
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

      <Dialog open={inviteDialogOpen} onOpenChange={setInviteDialogOpen}>
        <DialogContent className="sm:max-w-xl">
          <DialogHeader>
            <DialogTitle>Create an invite</DialogTitle>
            <DialogDescription>A code someone can use to create a cloud account.</DialogDescription>
          </DialogHeader>

          <form className="grid gap-4" onSubmit={handleGenerateInvite}>
            <div className="grid gap-4 sm:grid-cols-2">
              <Field id="invite-prefix" label="Prefix">
                <Input
                  id="invite-prefix"
                  value={invitePrefix}
                  onChange={(event) => setInvitePrefix(event.target.value)}
                  className="font-mono"
                />
              </Field>
              <Field id="invite-max-redemptions" label="Uses">
                <Input
                  id="invite-max-redemptions"
                  inputMode="numeric"
                  value={inviteMaxRedemptions}
                  onChange={(event) => setInviteMaxRedemptions(event.target.value)}
                />
              </Field>
              <Field id="invite-expiry" label="Expires">
                <Input
                  id="invite-expiry"
                  type="datetime-local"
                  value={inviteExpiry}
                  onChange={(event) => setInviteExpiry(event.target.value)}
                />
              </Field>
              <Field id="invite-note" label="Note">
                <Input
                  id="invite-note"
                  placeholder="beta tester"
                  value={inviteNote}
                  onChange={(event) => setInviteNote(event.target.value)}
                />
              </Field>
            </div>

            {lastInvite && (
              <div className="border-l-2 border-violet py-1 pl-4">
                <div className="label-caps">Share this code</div>
                <div className="mt-2 flex flex-wrap items-center justify-between gap-3">
                  <span className="font-mono text-base break-all">{lastInvite.code}</span>
                  <Button type="button" variant="outline" size="sm" onClick={handleCopyInvite}>
                    <Copy className="size-3.5" />
                    {copiedInvite ? "Copied" : "Copy"}
                  </Button>
                </div>
                <p className="mt-2 text-[13px] text-faint">
                  {plural(lastInvite.max_redemptions, "use")},{" "}
                  {lastInvite.expires_at ? `expires ${formatTimestamp(lastInvite.expires_at)}` : "never expires"}
                </p>
              </div>
            )}

            <DialogFooter>
              <Button type="button" variant="ghost" onClick={() => setInviteDialogOpen(false)}>
                Close
              </Button>
              <Button type="submit" disabled={inviteBusy}>
                {inviteBusy ? <Loader2 className="size-4 animate-spin" /> : <KeyRound className="size-4" />}
                {inviteBusy ? "Creating" : "Create invite"}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>

      <Dialog
        open={restoreDialogOpen}
        onOpenChange={(open) => {
          setRestoreDialogOpen(open);
          if (!open) {
            setRestoreTarget(null);
          }
        }}
      >
        <DialogContent className="sm:max-w-lg">
          <DialogHeader>
            <DialogTitle>Restore this backup?</DialogTitle>
            <DialogDescription>
              It replaces the library, sessions, event logs and covers on this PC.
            </DialogDescription>
          </DialogHeader>

          {restoreTarget && (
            <div className="grid gap-3">
              <p className="text-[15px]">
                {restoreTarget.label ?? "Backup"}, uploaded {formatTimestamp(restoreTarget.uploaded_at)}
              </p>
              <p className="font-mono text-sm text-soft">
                {restoreTarget.metadata_json
                  ? `${plural(restoreTarget.metadata_json.games_count, "game")}, ${plural(restoreTarget.metadata_json.sessions_count, "session")}`
                  : "Contents unknown"}
              </p>
              <p className="text-[13px] leading-relaxed text-faint">
                The download is checked against its checksum first. Close running games before you restore. Vaultime
                asks for a restart afterwards so tracking picks up cleanly.
              </p>
            </div>
          )}

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => setRestoreDialogOpen(false)}>
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => void handleRestoreRemoteBackup()}
              disabled={remoteBackupBusy || !restoreTarget}
            >
              {remoteBackupBusy ? <Loader2 className="size-4 animate-spin" /> : <ArchiveRestore className="size-4" />}
              {remoteBackupBusy ? "Restoring" : "Replace with this backup"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={deleteDialogOpen} onOpenChange={setDeleteDialogOpen}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete this backup?</DialogTitle>
            <DialogDescription>
              {deleteTarget?.label ?? "The backup"}, {deleteTarget ? backupSize(deleteTarget) : ""},
              is removed from the server. Nothing on this PC changes.
            </DialogDescription>
          </DialogHeader>

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => setDeleteDialogOpen(false)}>
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => void handleDeleteRemoteBackup()}
              disabled={remoteBackupBusy || !deleteTarget}
            >
              {remoteBackupBusy ? <Loader2 className="size-4 animate-spin" /> : <Trash2 className="size-4" />}
              {remoteBackupBusy ? "Deleting" : "Delete backup"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
