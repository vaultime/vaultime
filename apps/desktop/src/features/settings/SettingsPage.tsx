// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as autostartEnabled,
} from "@tauri-apps/plugin-autostart";
import { relaunch } from "@tauri-apps/plugin-process";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, Eye, RotateCcw, ScrollText } from "lucide-react";
import { Notice, PageHeader, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useLibrary } from "@/features/library/library-context";
import { useSettings } from "@/features/settings/settings-context";
import { AppearanceSection } from "@/features/settings/AppearanceSection";
import { CloudBackupSection } from "@/features/settings/CloudBackupSection";
import { DetectionSection } from "@/features/settings/DetectionSection";
import { EarlierPlaytimeSection } from "@/features/settings/EarlierPlaytimeSection";
import { ExportSection } from "@/features/settings/ExportSection";
import { IdleStepper } from "@/features/settings/IdleStepper";
import { IgnoredSection } from "@/features/settings/IgnoredSection";
import { LocalBackupsSection } from "@/features/settings/LocalBackupsSection";
import { NoticesDialog } from "@/features/settings/NoticesDialog";
import { UpdatesRow } from "@/features/settings/UpdatesRow";
import { OtherPcsSection } from "@/features/settings/OtherPcsSection";
import { ThisPcSection } from "@/features/settings/ThisPcSection";
import { WindowSection } from "@/features/settings/WindowSection";
import {
  COPYRIGHT_NOTICE,
  DEFAULT_IDLE_THRESHOLD_SECS,
  IDLE_SAVE_DELAY_MS,
  SETTING_KEYS,
  VAULTIME_URL,
} from "@/lib/constants";
import * as api from "@/lib/tauri";
import type { BackupSnapshot, TrackingDiagnostics } from "@/lib/types";
import { describeError, logFailure } from "@/lib/utils";
import { useScrollToAnchor } from "@/lib/anchors";
import { numberWords } from "@/lib/words";

const LINKS = [
  { label: "Website", url: VAULTIME_URL },
  { label: "Changelog", url: `${VAULTIME_URL}/changelog.html` },
  { label: "Privacy policy", url: `${VAULTIME_URL}/privacy.html` },
  { label: "Terms of service", url: `${VAULTIME_URL}/terms.html` },
];

const PLATFORM_NAMES: Record<string, string> = { windows: "Windows", linux: "Linux", macos: "macOS" };

export function SettingsPage() {
  const { refresh, summaries } = useLibrary();
  const { values, save } = useSettings();
  // Read once when the page opens, as the switches keep their own state.
  const [initial] = useState(() => {
    const seconds = Number(values[SETTING_KEYS.idleThreshold] ?? DEFAULT_IDLE_THRESHOLD_SECS);
    return {
      idleSeconds: Number.isFinite(seconds) ? seconds : DEFAULT_IDLE_THRESHOLD_SECS,
      closeToTray: values[SETTING_KEYS.closeToTray] !== "false",
      autoDiscover: values[SETTING_KEYS.autoDiscover] !== "false",
      autoBackup: values[SETTING_KEYS.autoBackup] !== "false",
      backgroundActive: values[SETTING_KEYS.backgroundActive] === "true",
    };
  });
  const [loaded, setLoaded] = useState(false);
  // What the command palette asked for, like ?do=backup, once the page is there.
  const [params, setParams] = useSearchParams();
  const intent = loaded ? params.get("do") : null;
  const [error, setError] = useState<string | null>(null);
  const [idleSeconds, setIdleSeconds] = useState(initial.idleSeconds);
  // The idle time saved last, and a change still waiting to be saved.
  const savedIdle = useRef(initial.idleSeconds);
  const pendingIdle = useRef<{ seconds: number; timer: number } | null>(null);
  const [backgroundActive, setBackgroundActive] = useState(initial.backgroundActive);
  const [diagnostics, setDiagnostics] = useState<TrackingDiagnostics | null>(null);
  const [backups, setBackups] = useState<{ autoBackup: boolean; folder: string; snapshots: BackupSnapshot[] }>({
    autoBackup: true,
    folder: "",
    snapshots: [],
  });
  const [appVersion, setAppVersion] = useState<string | null>(null);
  const [restartRequired, setRestartRequired] = useState(false);
  const [showingNotices, setShowingNotices] = useState(false);
  const [trayAvailable, setTrayAvailable] = useState(false);
  const [closeToTray, setCloseToTray] = useState(initial.closeToTray);
  const [autostart, setAutostart] = useState(false);
  const [autoDiscover, setAutoDiscover] = useState(initial.autoDiscover);

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      api.getTrackingDiagnostics(),
      api.listBackupSnapshots(),
      api.getAppVersion(),
      // Optional extras. Failing here must not hide the tracking rules.
      api.trayAvailable().catch(() => false),
      autostartEnabled().catch(() => false),
      api.getAutoBackupFolder().catch(() => ""),
    ])
      .then(([nextDiagnostics, nextSnapshots, version, tray, startsAtLogin, backupFolder]) => {
        if (cancelled) return;
        setTrayAvailable(Boolean(tray));
        setBackups({ autoBackup: initial.autoBackup, folder: backupFolder, snapshots: nextSnapshots });
        setAutostart(Boolean(startsAtLogin));
        setDiagnostics(nextDiagnostics);
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
  }, [initial.autoBackup]);

  // A change still waiting when the page closes is saved right away.
  useEffect(
    () => () => {
      const pending = pendingIdle.current;
      if (!pending) return;
      pendingIdle.current = null;
      window.clearTimeout(pending.timer);
      save(SETTING_KEYS.idleThreshold, String(pending.seconds)).catch(logFailure("saving the idle time"));
    },
    [save],
  );

  /** Saves the idle time shortly after the last change, so stepping through values saves once. */
  function changeIdle(next: number) {
    setIdleSeconds(next);
    if (pendingIdle.current) window.clearTimeout(pendingIdle.current.timer);
    const timer = window.setTimeout(() => {
      pendingIdle.current = null;
      save(SETTING_KEYS.idleThreshold, String(next))
        .then(() => {
          savedIdle.current = next;
          setError(null);
        })
        .catch((saveError) => {
          // A newer change still waiting stays in the field.
          if (!pendingIdle.current) setIdleSeconds(savedIdle.current);
          setError(describeError(saveError));
        });
    }, IDLE_SAVE_DELAY_MS);
    pendingIdle.current = { seconds: next, timer };
  }

  /** Shows a switch's new state at once and turns it back when saving fails. */
  async function toggle(set: (value: boolean) => void, next: boolean, save: () => Promise<unknown>) {
    set(next);
    try {
      await save();
    } catch (saveError) {
      set(!next);
      setError(describeError(saveError));
    }
  }
  const saveSetting = (key: string, next: boolean) => () => save(key, String(next));

  useScrollToAnchor(loaded);
  // A request from the palette runs once the page is there, also when
  // Settings is open already, then leaves the address, so going back does
  // not run it again. Backups and exports start in their own sections.
  useEffect(() => {
    if (!intent) return;
    const timer = setTimeout(() => {
      setParams(
        (next) => {
          next.delete("do");
          return next;
        },
        { replace: true },
      );
    });
    return () => clearTimeout(timer);
    // Once per request.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [intent]);

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
              onCheckedChange={(checked) =>
                void toggle(setBackgroundActive, checked, saveSetting(SETTING_KEYS.backgroundActive, checked))
              }
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
              onCheckedChange={(checked) =>
                void toggle(setCloseToTray, checked, saveSetting(SETTING_KEYS.closeToTray, checked))
              }
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
            <Switch
              id="autostart"
              checked={autostart}
              onCheckedChange={(checked) =>
                void toggle(setAutostart, checked, () => (checked ? enableAutostart() : disableAutostart()))
              }
            />
          </PageRow>
          <PageRow
            label="Look for new games when Vaultime starts"
            htmlFor="auto-discover"
            hint="Reads what Steam and your other launchers installed and asks only about games it has not offered before. Discover in the library still lists everything."
          >
            <Switch
              id="auto-discover"
              checked={autoDiscover}
              onCheckedChange={(checked) =>
                void toggle(setAutoDiscover, checked, saveSetting(SETTING_KEYS.autoDiscover, checked))
              }
            />
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

        <IgnoredSection onError={setError} />

        <EarlierPlaytimeSection onError={setError} />

        <DetectionSection diagnostics={diagnostics} platform={platform} />

        <ThisPcSection onError={setError} />

        <OtherPcsSection onError={setError} />

        <LocalBackupsSection
          initial={backups}
          start={intent === "backup"}
          onError={setError}
          onRestartRequired={() => setRestartRequired(true)}
        />

        <CloudBackupSection onError={setError} />

        <ExportSection start={intent === "export-csv" ? "csv" : intent === "export-json" ? "json" : null} />

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
            <Button variant="outline" size="sm" onClick={() => setShowingNotices(true)}>
              <ScrollText className="size-3.5" />
              Third-party notices
            </Button>
          </div>
          {showingNotices && <NoticesDialog onClose={() => setShowingNotices(false)} />}
        </PageSection>
      </div>
    </div>
  );
}
