// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { open as shellOpen } from "@tauri-apps/plugin-shell";
import {
  ArchiveRestore,
  Download,
  ExternalLink,
  HardDriveDownload,
  Info,
  Laptop2,
  Loader2,
  RotateCcw,
  Save,
  Settings,
  TimerReset,
  Upload,
} from "lucide-react";
import { useEffect, useState } from "react";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { DEFAULT_IDLE_THRESHOLD_SECONDS } from "@/lib/constants";
import { formatLongDate, formatSessionDate } from "@/lib/time";
import type {
  BackupSnapshot,
  LocalBackupSummary,
  Setting,
  TrackingDiagnostics,
} from "@/lib/types";
import * as api from "@/lib/tauri";

function mapSettings(settings: Setting[]): Record<string, string> {
  const values: Record<string, string> = {};
  for (const setting of settings) {
    values[setting.key] = setting.value;
  }
  return values;
}

function describeForegroundDetection(
  diagnostics: TrackingDiagnostics | null,
): { title: string; description: string } {
  switch (diagnostics?.foreground_detection) {
    case "x11":
      return {
        title: "X11 window PID",
        description:
          "Vaultime can map the active X11 window back to a tracked game process.",
      };
    case "win32_api":
      return {
        title: "Win32 foreground window",
        description:
          "Vaultime is reading the active top-level window directly from the Windows API.",
      };
    case "macos_system":
      return {
        title: "macOS frontmost process",
        description:
          "Vaultime is asking the macOS windowing layer for the frontmost application process.",
      };
    default:
      return {
        title: "Process heuristic",
        description:
          "Vaultime is falling back to recent process activity instead of a direct active-window signal.",
      };
  }
}

function describeIdleDetection(
  diagnostics: TrackingDiagnostics | null,
): { title: string; description: string } {
  switch (diagnostics?.idle_detection) {
    case "x11":
      return {
        title: "X11 idle timer",
        description:
          "System idle time is available, so background sessions stop counting as active after the configured threshold.",
      };
    case "win32_api":
      return {
        title: "Win32 last-input timer",
        description:
          "Vaultime is reading the last keyboard or mouse input timestamp directly from Windows.",
      };
    case "macos_ioreg":
      return {
        title: "macOS HID idle timer",
        description:
          "Vaultime is using the macOS HID idle counter to tell active play from idle/background time.",
      };
    default:
      return {
        title: "Process heuristic",
        description:
          "Vaultime is using process activity gaps as a conservative fallback for idle/background time.",
      };
  }
}

function platformCoverageNote(platform: string | undefined): string {
  switch (platform) {
    case "linux":
      return "Linux can use X11-specific tools when available, with heuristics as fallback.";
    case "windows":
      return "Windows uses Win32 foreground and idle APIs for native tracking signals.";
    case "macos":
      return "macOS uses frontmost-process and HID idle probes where the system allows them.";
    default:
      return "Vaultime prefers native platform signals and falls back to process heuristics when they are unavailable.";
  }
}

export function SettingsPage() {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [backupBusy, setBackupBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [savedMessage, setSavedMessage] = useState<string | null>(null);
  const [backupMessage, setBackupMessage] = useState<string | null>(null);
  const [idleThresholdSeconds, setIdleThresholdSeconds] = useState(String(DEFAULT_IDLE_THRESHOLD_SECONDS));
  const [treatBackgroundAsActive, setTreatBackgroundAsActive] = useState(false);
  const [diagnostics, setDiagnostics] =
    useState<TrackingDiagnostics | null>(null);
  const [backupSnapshots, setBackupSnapshots] = useState<BackupSnapshot[]>([]);
  const [restorePreview, setRestorePreview] =
    useState<LocalBackupSummary | null>(null);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [restartRequired, setRestartRequired] = useState(false);

  async function refreshBackupSnapshots() {
    const snapshots = await api.listBackupSnapshots();
    setBackupSnapshots(snapshots);
  }

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        setLoading(true);
        setError(null);

        const [settings, trackingDiagnostics, snapshots, version] = await Promise.all([
          api.listSettings(),
          api.getTrackingDiagnostics(),
          api.listBackupSnapshots(),
          api.getAppVersion(),
        ]);

        if (cancelled) {
          return;
        }

        const values = mapSettings(settings);
        setIdleThresholdSeconds(values.idle_threshold_seconds ?? String(DEFAULT_IDLE_THRESHOLD_SECONDS));
        setTreatBackgroundAsActive(
          values.treat_background_as_active === "true",
        );
        setDiagnostics(trackingDiagnostics);
        setBackupSnapshots(snapshots);
        setAppVersion(version);
      } catch (loadError) {
        if (!cancelled) {
          setError(String(loadError));
        }
      } finally {
        if (!cancelled) {
          setLoading(false);
        }
      }
    }

    void load();

    return () => {
      cancelled = true;
    };
  }, []);

  async function handleSave() {
    const parsedThreshold = Number.parseInt(idleThresholdSeconds, 10);
    if (!Number.isFinite(parsedThreshold) || parsedThreshold < 5) {
      setError("Idle threshold must be a whole number of at least 5 seconds.");
      return;
    }

    try {
      setSaving(true);
      setError(null);
      setSavedMessage(null);

      await api.setSetting(
        "idle_threshold_seconds",
        String(parsedThreshold),
      );
      await api.setSetting(
        "treat_background_as_active",
        String(treatBackgroundAsActive),
      );

      setSavedMessage("Tracking rules saved.");
    } catch (saveError) {
      setError(String(saveError));
    } finally {
      setSaving(false);
    }
  }

  async function handleExportBackup() {
    const selected = await open({
      multiple: false,
      directory: true,
      title: "Choose backup destination folder",
    });

    if (!selected || typeof selected !== "string") {
      return;
    }

    try {
      setBackupBusy(true);
      setError(null);
      setBackupMessage(null);

      const summary = await api.exportLocalBackup(selected);
      setBackupMessage(`Backup exported to ${summary.backup_path}.`);
      setRestorePreview(null);
      await refreshBackupSnapshots();
    } catch (backupError) {
      setError(String(backupError));
    } finally {
      setBackupBusy(false);
    }
  }

  async function handleChooseRestoreBackup() {
    const selected = await open({
      multiple: false,
      directory: true,
      title: "Choose Vaultime backup folder",
    });

    if (!selected || typeof selected !== "string") {
      return;
    }

    try {
      setBackupBusy(true);
      setError(null);
      setBackupMessage(null);

      const summary = await api.inspectLocalBackup(selected);
      setRestorePreview(summary);
    } catch (backupError) {
      setError(String(backupError));
    } finally {
      setBackupBusy(false);
    }
  }

  async function handleRestoreBackup() {
    if (!restorePreview) {
      return;
    }

    try {
      setBackupBusy(true);
      setError(null);
      setBackupMessage(null);

      const summary = await api.importLocalBackup(restorePreview.backup_path);
      setRestorePreview(summary);
      setBackupMessage("Backup restored.");
      if (summary.restart_required) {
        setRestartRequired(true);
      }
      await refreshBackupSnapshots();
    } catch (backupError) {
      setError(String(backupError));
    } finally {
      setBackupBusy(false);
    }
  }

  async function handleRestart() {
    try {
      await relaunch();
    } catch (restartError) {
      setError(String(restartError));
    }
  }

  if (loading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-6 w-6 animate-spin text-muted-foreground" />
      </div>
    );
  }

  const foregroundDetails = describeForegroundDetection(diagnostics);
  const idleDetails = describeIdleDetection(diagnostics);

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-2xl font-bold tracking-tight">Settings</h1>
        <p className="text-muted-foreground">
          Tune active playtime rules, inspect platform signals, and manage local
          backup snapshots.
        </p>
      </div>

      {error && (
        <div className="rounded-lg border border-destructive/50 bg-destructive/10 p-4 text-sm text-destructive">
          {error}
        </div>
      )}

      {savedMessage && (
        <div className="rounded-lg border border-green-500/40 bg-green-500/10 p-4 text-sm text-green-500">
          {savedMessage}
        </div>
      )}

      {backupMessage && (
        <div className="rounded-lg border border-primary/40 bg-primary/10 p-4 text-sm text-primary">
          {backupMessage}
        </div>
      )}

      {restartRequired && (
        <div className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-primary/40 bg-primary/10 p-4 text-sm text-primary">
          Restart Vaultime to resume live tracking on this machine.
          <Button size="sm" onClick={() => void handleRestart()}>
            <RotateCcw className="h-3.5 w-3.5" />
            Restart now
          </Button>
        </div>
      )}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <TimerReset className="h-4 w-4 text-primary" />
              Tracking Rules
            </CardTitle>
            <CardDescription>
              Active time requires a live tracked process and the rules below.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="space-y-2">
              <Label htmlFor="idle-threshold">
                Idle Threshold Seconds
              </Label>
              <Input
                id="idle-threshold"
                type="number"
                min={5}
                step={5}
                value={idleThresholdSeconds}
                onChange={(event) => {
                  setIdleThresholdSeconds(event.target.value);
                  setSavedMessage(null);
                }}
              />
              <p className="text-xs text-muted-foreground">
                When Vaultime sees no convincing activity for this long, it
                shifts the session from active time to idle/background time.
              </p>
            </div>

            <div className="rounded-xl border border-border/70 bg-muted/20 p-4">
              <div className="flex items-start justify-between gap-4">
                <div className="space-y-1">
                  <Label
                    htmlFor="background-active"
                    className="text-sm font-medium"
                  >
                    Count Background Runtime As Active
                  </Label>
                  <p className="text-xs leading-5 text-muted-foreground">
                    Turn this on if you want minimized or unfocused games to
                    keep earning active time while the machine is still in use.
                  </p>
                </div>
                <input
                  id="background-active"
                  type="checkbox"
                  checked={treatBackgroundAsActive}
                  onChange={(event) => {
                    setTreatBackgroundAsActive(event.target.checked);
                    setSavedMessage(null);
                  }}
                  className="mt-0.5 h-4 w-4 rounded border border-input bg-background accent-primary"
                />
              </div>
            </div>

            <div className="flex items-center justify-between gap-4">
              <p className="text-xs text-muted-foreground">
                Poll interval: {diagnostics?.poll_interval_seconds ?? 5}s
              </p>
              <Button onClick={handleSave} disabled={saving}>
                {saving ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Save className="h-4 w-4" />
                )}
                Save Rules
              </Button>
            </div>
          </CardContent>
        </Card>

        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Settings className="h-4 w-4 text-primary" />
              Detection Status
            </CardTitle>
            <CardDescription>
              Backend status and the signal sources currently available to the
              tracker.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/20 px-4 py-3">
              <span className="text-sm text-muted-foreground">Platform</span>
              <Badge variant="outline" className="border-primary/40 text-primary">
                {diagnostics?.platform ?? "unknown"}
              </Badge>
            </div>

            <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/20 px-4 py-3">
              <span className="text-sm text-muted-foreground">
                Tracking engine
              </span>
              <Badge
                variant="outline"
                className={
                  diagnostics?.running
                    ? "border-green-500/50 text-green-500"
                    : "border-yellow-500/50 text-yellow-500"
                }
              >
                {diagnostics?.running ? "Running" : "Stopped"}
              </Badge>
            </div>

            <div className="rounded-xl border border-border/70 bg-muted/20 p-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Foreground detection
              </p>
              <p className="mt-2 text-lg font-semibold">
                {foregroundDetails.title}
              </p>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                {foregroundDetails.description}
              </p>
            </div>

            <div className="rounded-xl border border-border/70 bg-muted/20 p-4">
              <p className="text-xs uppercase tracking-[0.2em] text-muted-foreground">
                Idle detection
              </p>
              <p className="mt-2 text-lg font-semibold">
                {idleDetails.title}
              </p>
              <p className="mt-1 text-xs leading-5 text-muted-foreground">
                {idleDetails.description}
              </p>
            </div>

            <p className="text-xs leading-5 text-muted-foreground">
              {platformCoverageNote(diagnostics?.platform)}
            </p>
          </CardContent>
        </Card>
      </div>

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(0,0.9fr)]">
        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <HardDriveDownload className="h-4 w-4 text-primary" />
              Local Backups
            </CardTitle>
            <CardDescription>
              Export a self-contained snapshot or inspect a backup before
              restoring it over this device&apos;s current library and history.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-6">
            <div className="flex flex-wrap gap-3">
              <Button onClick={handleExportBackup} disabled={backupBusy}>
                {backupBusy ? (
                  <Loader2 className="h-4 w-4 animate-spin" />
                ) : (
                  <Download className="h-4 w-4" />
                )}
                Export Backup
              </Button>
              <Button
                variant="outline"
                onClick={handleChooseRestoreBackup}
                disabled={backupBusy}
              >
                <Upload className="h-4 w-4" />
                Choose Backup To Restore
              </Button>
            </div>

            <div className="rounded-2xl border border-border/70 bg-muted/20 p-4 text-xs leading-6 text-muted-foreground">
              Exports include a consistent SQLite snapshot, cached artwork, and
              a manifest with per-file checksums. Restoring replaces the local
              library, sessions, integrity history, and cached covers on this
              machine.
            </div>

            {restorePreview && (
              <div className="space-y-4 rounded-2xl border border-amber-400/30 bg-amber-500/8 p-4">
                <div className="flex items-start justify-between gap-4">
                  <div>
                    <p className="text-sm font-semibold text-foreground">
                      Restore Preview
                    </p>
                    <p className="mt-1 text-xs leading-5 text-muted-foreground">
                      Backup from {formatLongDate(restorePreview.created_at)} on{" "}
                      {restorePreview.source_device_id}.
                    </p>
                  </div>
                  <Badge
                    variant="outline"
                    className="border-amber-400/40 text-amber-200"
                  >
                    Destructive
                  </Badge>
                </div>

                <div className="grid gap-3 sm:grid-cols-3">
                  <div className="rounded-xl border border-border/70 bg-background/35 p-3">
                    <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                      Games
                    </p>
                    <p className="mt-2 text-lg font-semibold">
                      {restorePreview.games_count}
                    </p>
                  </div>
                  <div className="rounded-xl border border-border/70 bg-background/35 p-3">
                    <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                      Sessions
                    </p>
                    <p className="mt-2 text-lg font-semibold">
                      {restorePreview.sessions_count}
                    </p>
                  </div>
                  <div className="rounded-xl border border-border/70 bg-background/35 p-3">
                    <p className="text-[10px] uppercase tracking-[0.2em] text-muted-foreground">
                      Cached Assets
                    </p>
                    <p className="mt-2 text-lg font-semibold">
                      {restorePreview.asset_file_count}
                    </p>
                  </div>
                </div>

                <div className="rounded-xl border border-border/70 bg-background/35 p-4 text-xs leading-6 text-muted-foreground">
                  Restoring will overwrite this device&apos;s current local
                  history. Close any live sessions first. Vaultime will require
                  a restart after restore so tracking can resume cleanly.
                </div>

                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="text-xs text-muted-foreground">
                    Manifest checksum: {restorePreview.overall_checksum.slice(0, 16)}...
                  </div>
                  <Button
                    variant="destructive"
                    onClick={handleRestoreBackup}
                    disabled={backupBusy}
                  >
                    {backupBusy ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : (
                      <ArchiveRestore className="h-4 w-4" />
                    )}
                    Restore Backup
                  </Button>
                </div>
              </div>
            )}
          </CardContent>
        </Card>

        <Card className="border border-border/70">
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Laptop2 className="h-4 w-4 text-primary" />
              Recent Snapshots
            </CardTitle>
            <CardDescription>
              Local export and restore checkpoints recorded in your local
              history.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            {backupSnapshots.length === 0 ? (
              <div className="rounded-2xl border border-dashed border-border/70 bg-muted/20 p-4 text-sm text-muted-foreground">
                No local backup snapshots recorded yet.
              </div>
            ) : (
              backupSnapshots.map((snapshot) => (
                <div
                  key={snapshot.id}
                  className="rounded-2xl border border-border/70 bg-muted/20 px-4 py-3"
                >
                  <div className="flex items-start justify-between gap-3">
                    <div>
                      <p className="text-sm font-medium">
                        {snapshot.restore_point_label ?? "Local snapshot"}
                      </p>
                      <p className="mt-1 text-xs text-muted-foreground">
                        {formatSessionDate(snapshot.created_at)}
                      </p>
                    </div>
                    <Badge variant="outline" className="border-border/70">
                      {snapshot.source_device_id ?? "unknown device"}
                    </Badge>
                  </div>
                  <p className="mt-3 truncate text-xs text-muted-foreground">
                    {snapshot.remote_path ?? "Path unavailable"}
                  </p>
                </div>
              ))
            )}
          </CardContent>
        </Card>
      </div>

      <Card className="border border-border/70">
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Info className="h-4 w-4 text-primary" />
            About Vaultime
          </CardTitle>
          <CardDescription>
            Application info and legal documents.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/20 px-4 py-3">
            <span className="text-sm text-muted-foreground">Version</span>
            <Badge variant="outline" className="border-primary/40 text-primary">
              {appVersion ?? "unknown"}
            </Badge>
          </div>

          <div className="flex items-center justify-between rounded-xl border border-border/70 bg-muted/20 px-4 py-3">
            <span className="text-sm text-muted-foreground">License</span>
            <span className="text-sm text-foreground">MIT</span>
          </div>

          <div className="flex flex-wrap gap-3">
            <Button
              variant="outline"
              size="sm"
              onClick={() => shellOpen("https://github.com/schwimmbeck/vaultime")}
            >
              <ExternalLink className="h-3.5 w-3.5" />
              GitHub
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => shellOpen("https://github.com/schwimmbeck/vaultime/blob/main/docs/legal/privacy-policy.md")}
            >
              <ExternalLink className="h-3.5 w-3.5" />
              Privacy Policy
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => shellOpen("https://github.com/schwimmbeck/vaultime/blob/main/docs/legal/terms-of-service.md")}
            >
              <ExternalLink className="h-3.5 w-3.5" />
              Terms of Service
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
