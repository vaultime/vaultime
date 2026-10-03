// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useRef, useState } from "react";
import { Link } from "react-router";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as autostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { relaunch } from "@tauri-apps/plugin-process";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ArchiveRestore, Download, ExternalLink, Eye, Loader2, RotateCcw, Upload } from "lucide-react";
import { Notice, PageHeader, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useLibrary } from "@/features/library/library-context";
import { AppearanceSection } from "@/features/settings/AppearanceSection";
import { EarlierPlaytimeSection } from "@/features/settings/EarlierPlaytimeSection";
import { ExportSection } from "@/features/settings/ExportSection";
import { IdleStepper } from "@/features/settings/IdleStepper";
import { UpdatesRow } from "@/features/settings/UpdatesRow";
import { WindowSection } from "@/features/settings/WindowSection";
import {
  AUTO_BACKUP_KEEP,
  COPYRIGHT_NOTICE,
  DEFAULT_IDLE_THRESHOLD_SECS,
  IDLE_SAVE_DELAY_MS,
  SETTING_KEYS,
  VAULTIME_URL,
} from "@/lib/constants";
import * as api from "@/lib/tauri";
import { formatLongDate, formatSessionStart } from "@/lib/time";
import type { BackupSnapshot, LocalBackupSummary, TrackingDiagnostics } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { capitalize, numberWords, plural, whichPc } from "@/lib/words";


const LINKS = [
  { label: "Website", url: VAULTIME_URL },
  { label: "Changelog", url: `${VAULTIME_URL}/changelog.html` },
  { label: "Privacy policy", url: `${VAULTIME_URL}/privacy.html` },
  { label: "Terms of service", url: `${VAULTIME_URL}/terms.html` },
];

const PLATFORM_NAMES: Record<string, string> = { windows: "Windows", linux: "Linux", macos: "macOS" };

const FOREGROUND: Record<string, { title: string; description: string }> = {
  win32_api: { title: "Windows foreground window", description: "Read straight from Windows." },
  x11: { title: "X11 active window", description: "The active X11 window is traced back to its process." },
  macos_system: { title: "macOS frontmost app", description: "Asked from the macOS window server." },
};

const IDLE: Record<string, { title: string; description: string }> = {
  win32_api: { title: "Windows last input", description: "The time since the last key press or mouse move." },
  x11_input: { title: "X11 input events", description: "The time since the last key press, click or mouse move." },
  x11: { title: "X11 idle timer", description: "The idle time X11 reports for this session." },
  gnome_dbus: { title: "GNOME idle monitor", description: "The time since the last input, from GNOME." },
  macos_ioreg: { title: "macOS input timer", description: "The idle counter of the input devices." },
};

const CONTROLLERS: Record<string, { title: string; description: string }> = {
  xinput: { title: "XInput", description: "Buttons and sticks of Xbox and other XInput controllers count as input." },
  evdev: { title: "Controller devices", description: "Buttons and sticks of every connected controller count as input." },
};

const NO_CONTROLLERS = { title: "Not read", description: "Only the keyboard and mouse count as input here." };

const FALLBACK = {
  title: "Process activity",
  description: "No direct signal here, so Vaultime judges by what the game process does.",
};

export function SettingsPage() {
  const { refresh, summaries } = useLibrary();
  const [loaded, setLoaded] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [idleSeconds, setIdleSeconds] = useState(DEFAULT_IDLE_THRESHOLD_SECS);
  // The idle time saved last, and a change still waiting to be saved.
  const savedIdle = useRef(DEFAULT_IDLE_THRESHOLD_SECS);
  const pendingIdle = useRef<{ seconds: number; timer: number } | null>(null);
  const [backgroundActive, setBackgroundActive] = useState(false);
  const [diagnostics, setDiagnostics] = useState<TrackingDiagnostics | null>(null);
  const [snapshots, setSnapshots] = useState<BackupSnapshot[]>([]);
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [backupBusy, setBackupBusy] = useState(false);
  const [backupMessage, setBackupMessage] = useState<string | null>(null);
  const [restorePreview, setRestorePreview] = useState<LocalBackupSummary | null>(null);
  const [restartRequired, setRestartRequired] = useState(false);
  const [trayAvailable, setTrayAvailable] = useState(false);
  const [closeToTray, setCloseToTray] = useState(true);
  const [autoBackup, setAutoBackup] = useState(true);
  const [autoBackupFolder, setAutoBackupFolder] = useState("");
  const [autostart, setAutostart] = useState(false);
  const [deviceId, setDeviceId] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      api.listSettings(),
      api.getTrackingDiagnostics(),
      api.listBackupSnapshots(),
      api.getAppVersion(),
      // Optional extras. Failing here must not hide the tracking rules.
      api.trayAvailable().catch(() => false),
      autostartEnabled().catch(() => false),
      api.getAutoBackupFolder().catch(() => ""),
      api.getDeviceId().catch(() => null),
    ])
      .then(([settings, nextDiagnostics, nextSnapshots, version, tray, startsAtLogin, backupFolder, thisDevice]) => {
        if (cancelled) return;
        const values = Object.fromEntries(settings.map((setting) => [setting.key, setting.value]));
        setTrayAvailable(Boolean(tray));
        setCloseToTray(values[SETTING_KEYS.closeToTray] !== "false");
        setAutoBackup(values[SETTING_KEYS.autoBackup] !== "false");
        setAutoBackupFolder(backupFolder);
        setDeviceId(thisDevice);
        setAutostart(Boolean(startsAtLogin));
        const seconds = Number(values[SETTING_KEYS.idleThreshold] ?? DEFAULT_IDLE_THRESHOLD_SECS);
        savedIdle.current = Number.isFinite(seconds) ? seconds : DEFAULT_IDLE_THRESHOLD_SECS;
        setIdleSeconds(savedIdle.current);
        setBackgroundActive(values[SETTING_KEYS.backgroundActive] === "true");
        setDiagnostics(nextDiagnostics);
        setSnapshots(nextSnapshots);
        setAppVersion(version);
      })
      .catch((loadError) => {
        if (!cancelled) setError(describeError(loadError));
      })
      .finally(() => {
        if (!cancelled) setLoaded(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const refreshLibrary = useRef(refresh);
  useEffect(() => {
    refreshLibrary.current = refresh;
  }, [refresh]);

  // A change still waiting when the page closes is saved right away.
  useEffect(
    () => () => {
      const pending = pendingIdle.current;
      if (!pending) return;
      pendingIdle.current = null;
      window.clearTimeout(pending.timer);
      void api
        .setSetting(SETTING_KEYS.idleThreshold, String(pending.seconds))
        .then(() => refreshLibrary.current(), () => {});
    },
    [],
  );

  /** Saves the idle time shortly after the last change, so stepping through values saves once. */
  function changeIdle(next: number) {
    setIdleSeconds(next);
    if (pendingIdle.current) window.clearTimeout(pendingIdle.current.timer);
    const timer = window.setTimeout(() => {
      pendingIdle.current = null;
      api
        .setSetting(SETTING_KEYS.idleThreshold, String(next))
        .then(() => {
          savedIdle.current = next;
          setError(null);
          // The live bar shows the idle threshold, so the library reloads it.
          refresh().catch(() => {});
        })
        .catch((saveError) => {
          // A newer change still waiting stays in the field.
          if (!pendingIdle.current) setIdleSeconds(savedIdle.current);
          setError(describeError(saveError));
        });
    }, IDLE_SAVE_DELAY_MS);
    pendingIdle.current = { seconds: next, timer };
  }

  async function changeBackgroundActive(next: boolean) {
    setBackgroundActive(next);
    try {
      await api.setSetting(SETTING_KEYS.backgroundActive, String(next));
    } catch (saveError) {
      setBackgroundActive(!next);
      setError(describeError(saveError));
    }
  }

  async function changeCloseToTray(next: boolean) {
    setCloseToTray(next);
    try {
      await api.setSetting(SETTING_KEYS.closeToTray, String(next));
    } catch (saveError) {
      setCloseToTray(!next);
      setError(describeError(saveError));
    }
  }

  async function changeAutoBackup(next: boolean) {
    setAutoBackup(next);
    try {
      await api.setSetting(SETTING_KEYS.autoBackup, String(next));
    } catch (saveError) {
      setAutoBackup(!next);
      setError(describeError(saveError));
    }
  }

  async function changeAutostart(next: boolean) {
    setAutostart(next);
    try {
      await (next ? enableAutostart() : disableAutostart());
    } catch (autostartError) {
      setAutostart(!next);
      setError(describeError(autostartError));
    }
  }

  async function runBackupTask(task: () => Promise<void>) {
    try {
      setBackupBusy(true);
      setError(null);
      setBackupMessage(null);
      await task();
    } catch (backupError) {
      setError(describeError(backupError));
    } finally {
      setBackupBusy(false);
    }
  }

  async function chooseFolder(title: string): Promise<string | null> {
    const selected = await openFileDialog({ multiple: false, directory: true, title });
    return typeof selected === "string" ? selected : null;
  }

  const exportBackup = () =>
    runBackupTask(async () => {
      const folder = await chooseFolder("Choose where to save the backup");
      if (!folder) return;
      const summary = await api.exportLocalBackup(folder);
      setBackupMessage(`Saved to ${summary.backup_path}`);
      setRestorePreview(null);
      setSnapshots(await api.listBackupSnapshots());
    });

  const changeAutoBackupFolder = () =>
    runBackupTask(async () => {
      const folder = await chooseFolder("Choose where automatic backups go");
      if (!folder) return;
      await api.setSetting(SETTING_KEYS.autoBackupFolder, folder);
      setAutoBackupFolder(await api.getAutoBackupFolder());
    });

  const chooseRestore = () =>
    runBackupTask(async () => {
      const folder = await chooseFolder("Choose a Vaultime backup folder");
      if (!folder) return;
      setRestorePreview(await api.inspectLocalBackup(folder));
    });

  const restore = () =>
    runBackupTask(async () => {
      if (!restorePreview) return;
      const summary = await api.importLocalBackup(restorePreview.backup_path);
      setRestorePreview(null);
      setBackupMessage("Backup restored.");
      if (summary.restart_required) setRestartRequired(true);
      setSnapshots(await api.listBackupSnapshots());
      await refresh();
    });

  if (!loaded) return null;

  const hidden = summaries.filter((summary) => summary.game.is_hidden);

  async function showInLibrary(gameId: string) {
    try {
      await api.updateGame(gameId, { is_hidden: false });
      await refresh();
    } catch (showError) {
      setError(describeError(showError));
    }
  }

  const platform = PLATFORM_NAMES[diagnostics?.platform ?? ""] ?? "this system";
  const foreground = FOREGROUND[diagnostics?.foreground_detection ?? ""] ?? FALLBACK;
  const idle = IDLE[diagnostics?.idle_detection ?? ""] ?? FALLBACK;
  const controllers = CONTROLLERS[diagnostics?.controller_detection ?? ""];
  const pollSeconds = diagnostics?.poll_interval_seconds;

  return (
    <div className="pb-16">
      <PageHeader overline={appVersion ? `Vaultime ${appVersion}` : "Vaultime"} title="Settings">
        {diagnostics?.running
          ? `Tracking runs on ${platform}${pollSeconds ? `, checking every ${numberWords(pollSeconds)} seconds` : ""}. Everything stays on this PC.`
          : "Tracking is stopped. Restart Vaultime to start it again."}
      </PageHeader>

      <div className="px-8 xl:px-14">
        {error && (
          <Notice tone="warning" className="mt-6">
            {error}
          </Notice>
        )}
        {restartRequired && (
          <Notice className="mt-6">
            Restart Vaultime to continue tracking with the restored history.
            <Button size="sm" onClick={() => relaunch().catch((restartError) => setError(describeError(restartError)))}>
              <RotateCcw className="size-3.5" />
              Restart now
            </Button>
          </Notice>
        )}

        <PageSection title="Tracking" description="How Vaultime tells playing apart from leaving a game open.">
          <PageRow
            label="Count as idle after"
            htmlFor="idle-minutes"
            hint="With no key press, mouse move or controller input for this long, time counts as idle instead of active."
          >
            <IdleStepper id="idle-minutes" seconds={idleSeconds} onChange={changeIdle} />
          </PageRow>
          <PageRow
            label="Count background games as active"
            htmlFor="background-active"
            hint="Minimized or unfocused games keep earning active time while you use the PC."
          >
            <Switch
              id="background-active"
              checked={backgroundActive}
              onCheckedChange={(checked) => void changeBackgroundActive(checked)}
            />
          </PageRow>
        </PageSection>

        <PageSection title="In the background" description="Vaultime counts games only while it runs.">
          <PageRow
            label="Keep tracking when the window is closed"
            htmlFor="close-to-tray"
            hint={
              trayAvailable
                ? "Vaultime stays in the tray. Quit it from the tray menu."
                : "This system has no tray, so closing the window quits Vaultime. On Linux, installing libayatana-appindicator3 adds one."
            }
          >
            <Switch
              id="close-to-tray"
              checked={trayAvailable && closeToTray}
              disabled={!trayAvailable}
              onCheckedChange={(checked) => void changeCloseToTray(checked)}
            />
          </PageRow>
          <PageRow
            label={diagnostics?.platform === "windows" ? "Start with Windows" : "Start when you log in"}
            htmlFor="autostart"
            hint={
              trayAvailable
                ? "Starts quietly in the tray, so the first game of the day counts too."
                : "Opens Vaultime when you log in."
            }
          >
            <Switch id="autostart" checked={autostart} onCheckedChange={(checked) => void changeAutostart(checked)} />
          </PageRow>
        </PageSection>

        <AppearanceSection />

        <WindowSection />

        {hidden.length > 0 && (
          <PageSection
            title="Hidden games"
            description="Still tracked, and their sessions count in the journal and your totals. Only the library leaves them out."
          >
            {hidden.map(({ game }) => (
              <PageRow
                key={game.id}
                label={
                  <Link to={`/library/${game.id}`} className="hover:underline">
                    {game.title}
                  </Link>
                }
              >
                <Button variant="outline" size="sm" onClick={() => void showInLibrary(game.id)}>
                  <Eye className="size-3.5" />
                  Show in library
                </Button>
              </PageRow>
            ))}
          </PageSection>
        )}

        <EarlierPlaytimeSection onError={setError} />

        <PageSection
          title="Detection"
          description="The signals this PC offers. Where a direct signal is missing, Vaultime falls back to process activity."
        >
          <PageRow label="System">{platform}</PageRow>
          <PageRow label="Tracking">
            <span className="flex items-center gap-2">
              <span className={diagnostics?.running ? "size-2 rounded-full bg-violet" : "size-2 rounded-full bg-amber"} />
              {diagnostics?.running ? "Running" : "Stopped"}
            </span>
          </PageRow>
          <PageRow label="Which window is in front" hint={foreground.description}>
            <span className="text-soft">{foreground.title}</span>
          </PageRow>
          <PageRow label="Whether you are there" hint={idle.description}>
            <span className="text-soft">{idle.title}</span>
          </PageRow>
          <PageRow label="Controllers" hint={(controllers ?? NO_CONTROLLERS).description}>
            <span className="text-soft">
              {(controllers ?? NO_CONTROLLERS).title}
              {controllers && diagnostics && (
                <span className="text-faint tabular-nums">, {diagnostics.controllers_connected} connected</span>
              )}
            </span>
          </PageRow>
          {pollSeconds && (
            <PageRow label="Checks every">
              <span className="font-mono">{pollSeconds} s</span>
            </PageRow>
          )}
        </PageSection>

        <PageSection
          title="Local backups"
          description="A backup is a folder with the database, the cached artwork and a checksum for every file."
        >
          <PageRow
            label="Back up automatically"
            htmlFor="auto-backup"
            hint={`Once a day and when Vaultime quits. The newest ${numberWords(AUTO_BACKUP_KEEP)} are kept, backups you save yourself are never deleted.`}
          >
            <Switch id="auto-backup" checked={autoBackup} onCheckedChange={(checked) => void changeAutoBackup(checked)} />
          </PageRow>
          <PageRow
            label={
              <>
                Automatic backups go to
                <span className="mt-1 block font-mono text-xs break-all text-soft">
                  {autoBackupFolder || "Not set"}
                </span>
              </>
            }
            hint="A folder on another drive or one that syncs keeps them safe if this drive fails."
          >
            <Button variant="outline" size="sm" onClick={changeAutoBackupFolder} disabled={backupBusy}>
              Change
            </Button>
          </PageRow>

          <div className="mt-6 flex flex-wrap gap-3">
            <Button onClick={exportBackup} disabled={backupBusy}>
              {backupBusy ? <Loader2 className="size-4 animate-spin" /> : <Download className="size-4" />}
              Save a backup
            </Button>
            <Button variant="outline" onClick={chooseRestore} disabled={backupBusy}>
              <Upload className="size-4" />
              Restore from a backup
            </Button>
          </div>
          {backupMessage && <Notice className="mt-5">{backupMessage}</Notice>}

          {restorePreview && (
            <div className="mt-6 border-l-2 border-amber py-1 pl-5">
              <p className="text-[15px]">
                Backup from {formatLongDate(restorePreview.created_at)}
                {whichPc(restorePreview.source_device_id, deviceId) &&
                  `, made on ${whichPc(restorePreview.source_device_id, deviceId)}`}
                .
              </p>
              <p className="mt-2 font-mono text-sm text-soft">
                {plural(restorePreview.games_count, "game")}, {plural(restorePreview.sessions_count, "session")},{" "}
                {plural(restorePreview.asset_file_count, "image")}
              </p>
              <p className="mt-3 max-w-[560px] text-[13px] leading-relaxed text-faint">
                Restoring replaces the library, sessions, event logs and covers on this PC. Close running games
                first. Vaultime asks for a restart afterwards so tracking picks up cleanly.
              </p>
              <div className="mt-4 flex items-center gap-4">
                <Button variant="destructive" onClick={restore} disabled={backupBusy}>
                  <ArchiveRestore className="size-4" />
                  Replace with this backup
                </Button>
                <Button variant="ghost" onClick={() => setRestorePreview(null)} disabled={backupBusy}>
                  Cancel
                </Button>
              </div>
            </div>
          )}

          <h3 className="label-caps mt-8 mb-1">History</h3>
          {snapshots.length === 0 ? (
            <p className="py-3 text-sm text-faint">No backups saved or restored yet.</p>
          ) : (
            snapshots.map((snapshot) => (
              <div key={snapshot.id} className="flex items-baseline gap-5 border-b border-rule py-3 last:border-b-0">
                <span className="w-[20ch] shrink-0 font-mono text-[13px] text-faint">
                  {formatSessionStart(snapshot.created_at)}
                </span>
                <div className="min-w-0 flex-1">
                  <div className="text-sm">{capitalize(snapshot.restore_point_label ?? "Local backup")}</div>
                  <div className="mt-0.5 truncate text-xs text-faint">
                    {[whichPc(snapshot.source_device_id, deviceId), snapshot.remote_path].filter(Boolean).join(", ")}
                  </div>
                </div>
              </div>
            ))
          )}
        </PageSection>

        <ExportSection />

        <PageSection title="About">
          <PageRow label="Version">
            <span className="font-mono">{appVersion ?? "unknown"}</span>
          </PageRow>
          <UpdatesRow />
          <PageRow label="Copyright">{COPYRIGHT_NOTICE}</PageRow>
          <PageRow label="License">GPL 3.0 or later</PageRow>
          <div className="mt-5 flex flex-wrap gap-2">
            {LINKS.map((link) => (
              <Button key={link.url} variant="outline" size="sm" onClick={() => openUrl(link.url)}>
                <ExternalLink className="size-3.5" />
                {link.label}
              </Button>
            ))}
          </div>
        </PageSection>
      </div>
    </div>
  );
}
