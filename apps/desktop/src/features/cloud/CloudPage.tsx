// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useEffectEvent, useState, type FormEvent } from "react";
import {
  ArchiveRestore,
  Cloud,
  Copy,
  HardDriveUpload,
  KeyRound,
  Loader2,
  LogOut,
  RefreshCw,
  Server,
  Shield,
  UserRound,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { useCloudSession } from "@/features/cloud/CloudSessionProvider";
import { formatLongDate } from "@/lib/time";
import type { CloudAdminInvite, CloudBackupRecord } from "@/lib/types";

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

function formatByteSize(bytes: number) {
  if (bytes <= 0) {
    return "0 B";
  }

  const units = ["B", "KB", "MB", "GB", "TB"];
  const exponent = Math.min(
    Math.floor(Math.log(bytes) / Math.log(1024)),
    units.length - 1,
  );
  const value = bytes / 1024 ** exponent;
  return `${value.toFixed(value >= 10 || exponent === 0 ? 0 : 1)} ${units[exponent]}`;
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

export function CloudPage() {
  const {
    apiBaseUrl,
    createAdminInvite,
    device,
    deviceError,
    initializing,
    isAdmin,
    listBackups,
    login,
    logout,
    refreshSession,
    registerCurrentDevice,
    restoreRemoteBackup,
    session,
    signUp,
    uploadRemoteBackup,
  } = useCloudSession();
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [statusMessage, setStatusMessage] = useState<string | null>(null);
  const [authBusy, setAuthBusy] = useState(false);
  const [deviceBusy, setDeviceBusy] = useState(false);
  const [backupsLoading, setBackupsLoading] = useState(false);
  const [remoteBackupBusy, setRemoteBackupBusy] = useState(false);
  const [inviteDialogOpen, setInviteDialogOpen] = useState(false);
  const [restoreDialogOpen, setRestoreDialogOpen] = useState(false);
  const [inviteBusy, setInviteBusy] = useState(false);
  const [lastInvite, setLastInvite] = useState<CloudAdminInvite | null>(null);
  const [copiedInvite, setCopiedInvite] = useState(false);
  const [remoteBackups, setRemoteBackups] = useState<CloudBackupRecord[]>([]);
  const [restoreTarget, setRestoreTarget] = useState<CloudBackupRecord | null>(null);
  const [loginEmail, setLoginEmail] = useState("");
  const [loginPassword, setLoginPassword] = useState("");
  const [signUpEmail, setSignUpEmail] = useState("");
  const [signUpPassword, setSignUpPassword] = useState("");
  const [inviteCode, setInviteCode] = useState("");
  const [invitePrefix, setInvitePrefix] = useState("VTLINV");
  const [inviteMaxRedemptions, setInviteMaxRedemptions] = useState("1");
  const [inviteExpiry, setInviteExpiry] = useState("");
  const [inviteNote, setInviteNote] = useState("");

  const loadBackups = useEffectEvent(async () => {
    try {
      setBackupsLoading(true);
      const backups = await listBackups();
      setRemoteBackups(backups);
    } catch (error) {
      setErrorMessage(String(error));
    } finally {
      setBackupsLoading(false);
    }
  });

  useEffect(() => {
    if (!session) {
      setRemoteBackups([]);
      return;
    }

    void loadBackups();
  }, [session]);

  async function handleLogin(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setAuthBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const nextSession = await login(loginEmail, loginPassword);
      setStatusMessage(`Signed in as ${nextSession.user.email}.`);
    } catch (error) {
      setErrorMessage(String(error));
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
      const nextSession = await signUp(
        signUpEmail,
        signUpPassword,
        inviteCode,
      );
      setStatusMessage(`Cloud account created for ${nextSession.user.email}.`);
      setInviteCode("");
    } catch (error) {
      setErrorMessage(String(error));
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
      setErrorMessage(String(error));
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
        setStatusMessage(`Device registered as ${registered.device_name}.`);
      }
    } catch (error) {
      setErrorMessage(String(error));
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
      setErrorMessage(String(error));
    } finally {
      setAuthBusy(false);
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
      setErrorMessage(String(error));
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
      setErrorMessage(String(error));
    }
  }

  async function handleReloadBackups() {
    setErrorMessage(null);
    try {
      setBackupsLoading(true);
      const backups = await listBackups();
      setRemoteBackups(backups);
    } catch (error) {
      setErrorMessage(String(error));
    } finally {
      setBackupsLoading(false);
    }
  }

  async function handleCreateRemoteBackup() {
    setRemoteBackupBusy(true);
    setErrorMessage(null);
    setStatusMessage(null);

    try {
      const result = await uploadRemoteBackup();
      setRemoteBackups((current) => [
        result.backup,
        ...current.filter((backup) => backup.id !== result.backup.id),
      ]);
      setStatusMessage(
        `Remote backup uploaded with ${result.payload_summary.games_count} games and ${result.payload_summary.sessions_count} sessions.`,
      );
    } catch (error) {
      setErrorMessage(String(error));
    } finally {
      setRemoteBackupBusy(false);
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
      setStatusMessage(
        result.restored_summary.restart_required
          ? "Remote backup restored. Restart Vaultime to resume live tracking on this machine."
          : "Remote backup restored.",
      );
    } catch (error) {
      setErrorMessage(String(error));
    } finally {
      setRemoteBackupBusy(false);
    }
  }

  function openRestoreDialog(backup: CloudBackupRecord) {
    setRestoreTarget(backup);
    setRestoreDialogOpen(true);
  }

  return (
    <div className="space-y-6">
      <div>
        <div className="flex items-center gap-2">
          <h1 className="text-2xl font-bold tracking-tight">Cloud</h1>
          <Badge
            variant="outline"
            className={
              session
                ? "border-emerald-500/40 text-emerald-300"
                : "border-border/60 text-muted-foreground"
            }
          >
            {session ? "Connected" : "Offline"}
          </Badge>
          {isAdmin && (
            <Badge variant="outline" className="border-amber-400/40 text-amber-200">
              Admin
            </Badge>
          )}
        </div>
        <p className="text-muted-foreground">
          Remote backup now talks to your self-hosted API on `codfishcloud.de`.
          Auth, device registration, remote backup upload/restore, and admin
          invite generation are live.
        </p>
      </div>

      {errorMessage && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
          {errorMessage}
        </div>
      )}

      {statusMessage && (
        <div className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-4 text-sm text-emerald-200">
          {statusMessage}
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        <Card className="relative overflow-hidden border border-border/70 bg-card/80">
          <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(139,92,246,0.18),transparent_34%),radial-gradient(circle_at_bottom_right,rgba(59,130,246,0.1),transparent_38%)]" />
          <CardHeader className="relative">
            <CardTitle className="flex items-center gap-2">
              <Cloud className="h-4 w-4 text-primary" />
              Remote connection
            </CardTitle>
            <CardDescription>
              The desktop app now talks to your VPS API directly for auth and
              invite-only cloud access.
            </CardDescription>
          </CardHeader>
          <CardContent className="relative grid gap-4 md:grid-cols-3">
            <div className="rounded-2xl border border-border/70 bg-background/45 p-4">
              <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                API host
              </p>
              <p className="mt-2 text-sm font-semibold tracking-tight">
                {formatApiHostname(apiBaseUrl)}
              </p>
              <p className="mt-1 break-all font-mono text-[11px] text-muted-foreground">
                {apiBaseUrl}
              </p>
            </div>
            <div className="rounded-2xl border border-border/70 bg-background/45 p-4">
              <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                Access model
              </p>
              <p className="mt-2 text-sm font-medium">
                Invite-only signup and admin-generated keys
              </p>
            </div>
            <div className="rounded-2xl border border-border/70 bg-background/45 p-4">
              <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                Session state
              </p>
              <p className="mt-2 text-sm font-semibold tracking-tight">
                {initializing
                  ? "Restoring secure session"
                  : session
                    ? "Signed in"
                    : "No cloud session"}
              </p>
              <p className="mt-1 break-all text-xs text-muted-foreground">
                {initializing
                  ? "Checking secure storage and refresh token state."
                  : session
                    ? `${session.user.email} · ${session.user.role}`
                    : "Sign in to enable remote backups and invite controls."}
              </p>
            </div>
          </CardContent>
        </Card>

        <Card className="border border-border/70 bg-card/80">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Server className="h-4 w-4 text-primary" />
              Current status
            </CardTitle>
            <CardDescription>
              Cloud auth, device registration, and remote backup transfers now
              run against your VPS API.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4 text-sm text-muted-foreground">
            <div className="flex items-start gap-3 rounded-2xl border border-border/70 bg-muted/20 p-4">
              <Shield className="mt-0.5 h-4 w-4 shrink-0 text-violet-300" />
              <p>
                Tokens are issued by the self-hosted API, then stored through
                OS secure storage instead of browser-local session state.
              </p>
            </div>
            <div className="flex items-start gap-3 rounded-2xl border border-border/70 bg-muted/20 p-4">
              <UserRound className="mt-0.5 h-4 w-4 shrink-0 text-violet-300" />
              <p>
                The current device registers against the account after sign-in
                so the API can associate future backups with this machine.
              </p>
            </div>
            <div className="flex items-start gap-3 rounded-2xl border border-border/70 bg-muted/20 p-4">
              <HardDriveUpload className="mt-0.5 h-4 w-4 shrink-0 text-violet-300" />
              <p>
                Backup archives are created in Rust, encrypted on the desktop
                before upload, and verified by SHA-256 on the VPS before the
                ciphertext is accepted.
              </p>
            </div>
            <div className="flex items-start gap-3 rounded-2xl border border-border/70 bg-muted/20 p-4">
              <Shield className="mt-0.5 h-4 w-4 shrink-0 text-violet-300" />
              <p>
                The current backup key is derived locally from the login
                credentials and retained in OS secure storage, which protects
                backup blobs at rest on the VPS. A separate zero-knowledge
                backup passphrase is still future hardening.
              </p>
            </div>
          </CardContent>
        </Card>
      </div>

      {!session ? (
        <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
          <Card className="border border-border/70 bg-card/80">
            <CardHeader>
              <CardTitle>Sign In</CardTitle>
              <CardDescription>
                Use an existing cloud account. The admin bootstrap account can
                sign in here too.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form className="space-y-4" onSubmit={handleLogin}>
                <div className="space-y-2">
                  <Label htmlFor="cloud-login-email">Email</Label>
                  <Input
                    id="cloud-login-email"
                    autoComplete="email"
                    value={loginEmail}
                    onChange={(event) => setLoginEmail(event.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="cloud-login-password">Password</Label>
                  <Input
                    id="cloud-login-password"
                    type="password"
                    autoComplete="current-password"
                    value={loginPassword}
                    onChange={(event) => setLoginPassword(event.target.value)}
                  />
                </div>
                <Button
                  type="submit"
                  className="w-full"
                  disabled={authBusy || !loginEmail.trim() || !loginPassword}
                >
                  {authBusy ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Signing in
                    </>
                  ) : (
                    "Sign In"
                  )}
                </Button>
              </form>
            </CardContent>
          </Card>

          <Card className="border border-border/70 bg-card/80">
            <CardHeader>
              <CardTitle>Create Cloud Account</CardTitle>
              <CardDescription>
                Sign up with a shareable invite key that was generated on your
                VPS or by an admin account.
              </CardDescription>
            </CardHeader>
            <CardContent>
              <form className="space-y-4" onSubmit={handleSignUp}>
                <div className="space-y-2">
                  <Label htmlFor="cloud-signup-email">Email</Label>
                  <Input
                    id="cloud-signup-email"
                    autoComplete="email"
                    value={signUpEmail}
                    onChange={(event) => setSignUpEmail(event.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="cloud-signup-password">Password</Label>
                  <Input
                    id="cloud-signup-password"
                    type="password"
                    autoComplete="new-password"
                    value={signUpPassword}
                    onChange={(event) => setSignUpPassword(event.target.value)}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="cloud-signup-invite">Invite code</Label>
                  <Input
                    id="cloud-signup-invite"
                    placeholder="VTLINV-XXXX-XXXX-XXXX-XXXX-XXXX-XXXX"
                    value={inviteCode}
                    onChange={(event) => setInviteCode(event.target.value)}
                  />
                </div>
                <Button
                  type="submit"
                  className="w-full"
                  disabled={
                    authBusy ||
                    !signUpEmail.trim() ||
                    !signUpPassword ||
                    !inviteCode.trim()
                  }
                >
                  {authBusy ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      Creating account
                    </>
                  ) : (
                    "Create Account"
                  )}
                </Button>
              </form>
            </CardContent>
          </Card>
        </div>
      ) : (
        <>
          <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_minmax(0,1fr)]">
            <Card className="border border-border/70 bg-card/80">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <UserRound className="h-4 w-4 text-primary" />
                  Account
                </CardTitle>
                <CardDescription>
                  Your current session is stored locally and refreshed from the
                  VPS API as needed.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                <div className="grid gap-3 sm:grid-cols-2">
                  <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                    <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                      Email
                    </p>
                    <p className="mt-2 text-sm font-medium">{session.user.email}</p>
                  </div>
                  <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                    <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                      Role
                    </p>
                    <p className="mt-2 text-sm font-medium">{session.user.role}</p>
                  </div>
                  <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                    <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                      Access token
                    </p>
                    <p className="mt-2 text-sm font-medium">
                      Expires {formatTimestamp(session.expires_at)}
                    </p>
                  </div>
                  <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                    <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                      Refresh token
                    </p>
                    <p className="mt-2 text-sm font-medium">
                      Expires {formatTimestamp(session.refresh_expires_at)}
                    </p>
                  </div>
                </div>
                <div className="flex flex-wrap gap-3">
                  <Button
                    variant="outline"
                    onClick={handleRefreshSession}
                    disabled={authBusy}
                  >
                    {authBusy ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <RefreshCw className="h-4 w-4" />
                    )}
                    Refresh session
                  </Button>
                  <Button variant="destructive" onClick={handleLogout} disabled={authBusy}>
                    <LogOut className="h-4 w-4" />
                    Sign out
                  </Button>
                </div>
              </CardContent>
            </Card>

            <Card className="border border-border/70 bg-card/80">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <Server className="h-4 w-4 text-primary" />
                  Device registration
                </CardTitle>
                <CardDescription>
                  The VPS tracks this desktop via a stable `client_device_id`.
                </CardDescription>
              </CardHeader>
              <CardContent className="space-y-4">
                {device ? (
                  <div className="grid gap-3 sm:grid-cols-2">
                    <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                      <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                        Device
                      </p>
                      <p className="mt-2 text-sm font-medium">{device.device_name}</p>
                    </div>
                    <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                      <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                        Platform
                      </p>
                      <p className="mt-2 text-sm font-medium">{device.platform}</p>
                    </div>
                    <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                      <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                        Client device id
                      </p>
                      <p className="mt-2 break-all text-sm font-medium">
                        {device.client_device_id}
                      </p>
                    </div>
                    <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                      <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                        Last seen
                      </p>
                      <p className="mt-2 text-sm font-medium">
                        {formatTimestamp(device.last_seen_at)}
                      </p>
                    </div>
                  </div>
                ) : (
                  <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                    No device registration has been confirmed yet.
                  </div>
                )}

                {deviceError && (
                  <div className="rounded-2xl border border-amber-500/30 bg-amber-500/10 p-4 text-sm text-amber-200">
                    Device registration warning: {deviceError}
                  </div>
                )}

                <Button
                  variant="outline"
                  onClick={handleRegisterDevice}
                  disabled={deviceBusy}
                >
                  {deviceBusy ? (
                    <Loader2 className="h-4 w-4 animate-spin" />
                  ) : (
                    <RefreshCw className="h-4 w-4" />
                  )}
                  Register this device
                </Button>
              </CardContent>
            </Card>
          </div>

          <Card className="border border-border/70 bg-card/80">
            <CardHeader>
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <CardTitle className="flex items-center gap-2">
                    <HardDriveUpload className="h-4 w-4 text-primary" />
                    Remote backup records
                  </CardTitle>
                  <CardDescription>
                    Records already stored on the VPS for this account.
                  </CardDescription>
                </div>
                <div className="flex flex-wrap gap-3">
                  <Button
                    onClick={() => void handleCreateRemoteBackup()}
                    disabled={remoteBackupBusy}
                  >
                    {remoteBackupBusy ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <HardDriveUpload className="h-4 w-4" />
                    )}
                    Create backup
                  </Button>
                  <Button
                    variant="outline"
                    onClick={() => void handleReloadBackups()}
                    disabled={backupsLoading || remoteBackupBusy}
                  >
                    <RefreshCw className="h-4 w-4" />
                    Reload
                  </Button>
                </div>
              </div>
            </CardHeader>
            <CardContent className="space-y-3">
              {backupsLoading ? (
                <div className="flex h-24 items-center justify-center rounded-2xl border border-border/70 bg-muted/20">
                  <Loader2 className="h-5 w-5 animate-spin text-muted-foreground" />
                </div>
              ) : remoteBackups.length === 0 ? (
                <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                  No remote backup records exist yet for this account.
                </div>
              ) : (
                remoteBackups.map((backup) => (
                  <div
                    key={backup.id}
                    className="rounded-2xl border border-border/70 bg-muted/20 p-4"
                  >
                    <div className="flex flex-wrap items-start justify-between gap-3">
                      <div>
                        <p className="text-sm font-medium">
                          {backup.label ?? "Unnamed backup"}
                        </p>
                        <p className="mt-1 text-xs text-muted-foreground">
                          Created {formatTimestamp(backup.backup_created_at)}
                        </p>
                      </div>
                      <Badge variant="outline" className="border-border/70 text-muted-foreground">
                        {backup.status}
                      </Badge>
                    </div>
                    <div className="mt-3 grid gap-3 sm:grid-cols-3">
                      <div>
                        <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                          Uploaded
                        </p>
                        <p className="mt-1 text-sm">{formatTimestamp(backup.uploaded_at)}</p>
                      </div>
                      <div>
                        <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                          Size
                        </p>
                        <p className="mt-1 text-sm">{formatByteSize(backup.size_bytes)}</p>
                      </div>
                      <div>
                        <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                          Source device
                        </p>
                        <p className="mt-1 break-all text-sm">
                          {backup.client_device_id ?? "Unknown"}
                        </p>
                      </div>
                    </div>
                    {backup.metadata_json && (
                      <div className="mt-3 grid gap-3 sm:grid-cols-2 xl:grid-cols-5">
                        <div>
                          <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                            Games
                          </p>
                          <p className="mt-1 text-sm">
                            {backup.metadata_json.games_count}
                          </p>
                        </div>
                        <div>
                          <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                            Sessions
                          </p>
                          <p className="mt-1 text-sm">
                            {backup.metadata_json.sessions_count}
                          </p>
                        </div>
                        <div>
                          <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                            Assets
                          </p>
                          <p className="mt-1 text-sm">
                            {backup.metadata_json.asset_file_count}
                          </p>
                        </div>
                        <div>
                          <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                            Source device
                          </p>
                          <p className="mt-1 text-sm">
                            {backup.metadata_json.source_device_id}
                          </p>
                        </div>
                        <div>
                          <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                            Encryption
                          </p>
                          <p className="mt-1 text-sm">
                            {backup.metadata_json.encryption ===
                            "chacha20poly1305-chunked-v1"
                              ? "Client-side encrypted"
                              : backup.metadata_json.encryption}
                          </p>
                        </div>
                      </div>
                    )}
                    <div className="mt-4 flex justify-end">
                      <Button
                        variant="outline"
                        onClick={() => openRestoreDialog(backup)}
                        disabled={remoteBackupBusy || backup.status !== "complete"}
                      >
                        <ArchiveRestore className="h-4 w-4" />
                        Restore to this device
                      </Button>
                    </div>
                  </div>
                ))
              )}
            </CardContent>
          </Card>

          {isAdmin && (
            <Card className="border border-border/70 bg-card/80">
              <CardHeader>
                <CardTitle className="flex items-center gap-2">
                  <KeyRound className="h-4 w-4 text-primary" />
                  Admin invite control
                </CardTitle>
                <CardDescription>
                  Generate a new shareable invite key for remote backup access.
                </CardDescription>
              </CardHeader>
              <CardContent className="flex flex-wrap items-center justify-between gap-4">
                <p className="max-w-2xl text-sm text-muted-foreground">
                  This uses the admin-only `/v1/admin/invites` endpoint. The raw
                  code is shown once here, while only its derived values are
                  stored on the VPS.
                </p>
                <Button onClick={() => setInviteDialogOpen(true)}>
                  <KeyRound className="h-4 w-4" />
                  Generate invite
                </Button>
              </CardContent>
            </Card>
          )}
        </>
      )}

      <Dialog open={inviteDialogOpen} onOpenChange={setInviteDialogOpen}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>Generate Invite</DialogTitle>
            <DialogDescription>
              Create a new invite code that you can share with a future cloud
              backup user.
            </DialogDescription>
          </DialogHeader>

          <form className="space-y-4" onSubmit={handleGenerateInvite}>
            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="invite-prefix">Prefix</Label>
                <Input
                  id="invite-prefix"
                  value={invitePrefix}
                  onChange={(event) => setInvitePrefix(event.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="invite-max-redemptions">Max redemptions</Label>
                <Input
                  id="invite-max-redemptions"
                  inputMode="numeric"
                  value={inviteMaxRedemptions}
                  onChange={(event) => setInviteMaxRedemptions(event.target.value)}
                />
              </div>
            </div>

            <div className="grid gap-4 sm:grid-cols-2">
              <div className="space-y-2">
                <Label htmlFor="invite-expiry">Expiry</Label>
                <Input
                  id="invite-expiry"
                  type="datetime-local"
                  value={inviteExpiry}
                  onChange={(event) => setInviteExpiry(event.target.value)}
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="invite-note">Note</Label>
                <Input
                  id="invite-note"
                  placeholder="beta tester"
                  value={inviteNote}
                  onChange={(event) => setInviteNote(event.target.value)}
                />
              </div>
            </div>

            {lastInvite && (
              <div className="rounded-2xl border border-emerald-500/30 bg-emerald-500/10 p-4">
                <div className="flex flex-wrap items-start justify-between gap-3">
                  <div>
                    <p className="text-[10px] uppercase tracking-[0.2em] text-emerald-200/80">
                      Share this code
                    </p>
                    <p className="mt-2 break-all font-mono text-base text-emerald-50">
                      {lastInvite.code}
                    </p>
                  </div>
                  <Button type="button" variant="outline" onClick={handleCopyInvite}>
                    <Copy className="h-4 w-4" />
                    {copiedInvite ? "Copied" : "Copy"}
                  </Button>
                </div>
                <div className="mt-4 grid gap-3 sm:grid-cols-2">
                  <div>
                    <p className="text-[10px] uppercase tracking-[0.2em] text-emerald-200/80">
                      Max redemptions
                    </p>
                    <p className="mt-1 text-sm text-emerald-50">
                      {lastInvite.max_redemptions}
                    </p>
                  </div>
                  <div>
                    <p className="text-[10px] uppercase tracking-[0.2em] text-emerald-200/80">
                      Expires
                    </p>
                    <p className="mt-1 text-sm text-emerald-50">
                      {formatTimestamp(lastInvite.expires_at)}
                    </p>
                  </div>
                </div>
              </div>
            )}

            <DialogFooter>
              <Button type="button" variant="ghost" onClick={() => setInviteDialogOpen(false)}>
                Close
              </Button>
              <Button type="submit" disabled={inviteBusy}>
                {inviteBusy ? (
                  <>
                    <Loader2 className="h-4 w-4 animate-spin" />
                    Generating
                  </>
                ) : (
                  <>
                    <KeyRound className="h-4 w-4" />
                    Generate invite
                  </>
                )}
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
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>Restore Remote Backup</DialogTitle>
            <DialogDescription>
              This replaces the current local library, sessions, integrity
              history, and cached artwork on this machine.
            </DialogDescription>
          </DialogHeader>

          {restoreTarget && (
            <div className="space-y-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Backup
                  </p>
                  <p className="mt-2 text-sm font-medium">
                    {restoreTarget.label ?? "Unnamed backup"}
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Uploaded
                  </p>
                  <p className="mt-2 text-sm font-medium">
                    {formatTimestamp(restoreTarget.uploaded_at)}
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Games
                  </p>
                  <p className="mt-2 text-sm font-medium">
                    {restoreTarget.metadata_json?.games_count ?? "Unknown"}
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-muted/20 p-4">
                  <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                    Sessions
                  </p>
                  <p className="mt-2 text-sm font-medium">
                    {restoreTarget.metadata_json?.sessions_count ?? "Unknown"}
                  </p>
                </div>
              </div>

              <div className="rounded-2xl border border-amber-400/30 bg-amber-500/8 p-4 text-sm text-muted-foreground">
                The desktop verifies the downloaded archive checksum before
                restore, but this is still a destructive action. Close any live
                sessions first. Vaultime will require a restart after restore so
                tracking can resume cleanly.
              </div>
            </div>
          )}

          <DialogFooter>
            <Button
              type="button"
              variant="ghost"
              onClick={() => setRestoreDialogOpen(false)}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={() => void handleRestoreRemoteBackup()}
              disabled={remoteBackupBusy || !restoreTarget}
            >
              {remoteBackupBusy ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  Restoring
                </>
              ) : (
                <>
                  <ArchiveRestore className="h-4 w-4" />
                  Restore backup
                </>
              )}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
