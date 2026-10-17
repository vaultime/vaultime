// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { ArchiveRestore, Download, Loader2, Upload } from "lucide-react";
import { Notice, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useLibrary } from "@/features/library/library-context";
import { useSettings } from "@/features/settings/settings-context";
import { TakeoverChoice } from "@/features/settings/TakeoverChoice";
import { AUTO_BACKUP_KEEP, SETTING_KEYS } from "@/lib/constants";
import { continuesAs } from "@/lib/devices";
import * as api from "@/lib/tauri";
import { formatLongDate, formatSessionStart } from "@/lib/time";
import type { BackupSnapshot, LocalBackupSummary } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { capitalize, numberWords, plural, whichPc } from "@/lib/words";

async function chooseFolder(title: string): Promise<string | null> {
  const selected = await openFileDialog({ multiple: false, directory: true, title });
  return typeof selected === "string" ? selected : null;
}

/** Automatic backups, saving and restoring a backup folder, and the backups so far. */
export function LocalBackupsSection({
  initial,
  start,
  onError,
  onRestartRequired,
}: {
  /** What the settings page read when it opened. */
  initial: { autoBackup: boolean; folder: string; snapshots: BackupSnapshot[] };
  /** Saves a backup right away, as asked for from the command palette. */
  start: boolean;
  onError: (message: string | null) => void;
  onRestartRequired: () => void;
}) {
  const { refresh, thisDeviceId } = useLibrary();
  const { save, reload: reloadSettings } = useSettings();
  const [autoBackup, setAutoBackup] = useState(initial.autoBackup);
  const [folder, setFolder] = useState(initial.folder);
  const [snapshots, setSnapshots] = useState(initial.snapshots);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [restorePreview, setRestorePreview] = useState<LocalBackupSummary | null>(null);
  // Taking over the backup's PC is right when this PC is that one set up again.
  const [takeOver, setTakeOver] = useState(true);

  async function changeAutoBackup(next: boolean) {
    setAutoBackup(next);
    try {
      await save(SETTING_KEYS.autoBackup, String(next));
    } catch (saveError) {
      setAutoBackup(!next);
      onError(describeError(saveError));
    }
  }

  async function run(task: () => Promise<void>) {
    try {
      setBusy(true);
      onError(null);
      setMessage(null);
      await task();
    } catch (backupError) {
      onError(describeError(backupError));
    } finally {
      setBusy(false);
    }
  }

  const exportBackup = () =>
    run(async () => {
      const picked = await chooseFolder("Choose where to save the backup");
      if (!picked) return;
      const summary = await api.exportLocalBackup(picked);
      setMessage(`Saved to ${summary.backup_path}`);
      setRestorePreview(null);
      setSnapshots(await api.listBackupSnapshots());
    });

  // Started outside the effect, so its folder dialog opens once the page is there.
  useEffect(() => {
    if (!start) return;
    const timer = setTimeout(() => void exportBackup());
    return () => clearTimeout(timer);
    // Once per request.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [start]);

  const changeFolder = () =>
    run(async () => {
      const picked = await chooseFolder("Choose where automatic backups go");
      if (!picked) return;
      await save(SETTING_KEYS.autoBackupFolder, picked);
      setFolder(await api.getAutoBackupFolder());
    });

  const chooseRestore = () =>
    run(async () => {
      const picked = await chooseFolder("Choose a Vaultime backup folder");
      if (!picked) return;
      setRestorePreview(await api.inspectLocalBackup(picked));
      setTakeOver(true);
    });

  const restore = () =>
    run(async () => {
      if (!restorePreview) return;
      const summary = await api.importLocalBackup(restorePreview.backup_path, takeOver);
      setRestorePreview(null);
      setMessage(summary.continues_as ? `Backup restored. ${continuesAs(summary.continues_as)}` : "Backup restored.");
      if (summary.restart_required) onRestartRequired();
      // A restore replaces the settings that are not kept per PC.
      await reloadSettings().catch(() => {});
      setSnapshots(await api.listBackupSnapshots());
      await refresh();
    });

  return (
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
            <span className="mt-1 block font-mono text-xs break-all text-soft">{folder || "Not set"}</span>
          </>
        }
        hint="A folder on another drive or one that syncs keeps them safe if this drive fails."
      >
        <Button variant="outline" size="sm" onClick={changeFolder} disabled={busy}>
          Change
        </Button>
      </PageRow>

      <div className="mt-6 flex flex-wrap gap-3">
        <Button onClick={exportBackup} disabled={busy}>
          {busy ? <Loader2 className="size-4 animate-spin" /> : <Download className="size-4" />}
          Save a backup
        </Button>
        <Button variant="outline" onClick={chooseRestore} disabled={busy}>
          <Upload className="size-4" />
          Restore from a backup
        </Button>
      </div>
      {message && <Notice className="mt-5">{message}</Notice>}

      {restorePreview && (
        <div className="mt-6 border-l-2 border-amber py-1 pl-5">
          <p className="text-[15px]">
            Backup from {formatLongDate(restorePreview.created_at)}
            {whichPc(restorePreview.source_device_id, thisDeviceId) &&
              `, made on ${whichPc(restorePreview.source_device_id, thisDeviceId)}`}
            .
          </p>
          <p className="mt-2 font-mono text-sm text-soft">
            {plural(restorePreview.games_count, "game")}, {plural(restorePreview.sessions_count, "session")},{" "}
            {plural(restorePreview.asset_file_count, "image")}
          </p>
          {restorePreview.continues_as && (
            <div className="mt-3">
              <TakeoverChoice
                id="take-over"
                takeover={restorePreview.continues_as}
                checked={takeOver}
                disabled={busy}
                onCheckedChange={setTakeOver}
              >
                <p className="text-[15px] leading-relaxed">
                  {continuesAs(restorePreview.continues_as)}{" "}
                  <span className="text-[13px] text-faint">
                    This PC has no history of its own yet, so it takes over the backup&apos;s PC: its sessions count
                    as this PC&apos;s own, and so do sessions it played after the backup that come in by merge, as
                    long as that PC checked them only with this PC&apos;s keys.
                  </span>
                </p>
              </TakeoverChoice>
            </div>
          )}
          <p className="mt-3 max-w-[560px] text-[13px] leading-relaxed text-faint">
            Restoring replaces the library, sessions, event logs and covers on this PC. Close running games first.
            Vaultime asks for a restart afterwards so tracking picks up cleanly.
          </p>
          <div className="mt-4 flex items-center gap-4">
            <Button variant="destructive" onClick={restore} disabled={busy}>
              <ArchiveRestore className="size-4" />
              Replace with this backup
            </Button>
            <Button variant="ghost" onClick={() => setRestorePreview(null)} disabled={busy}>
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
                {[whichPc(snapshot.source_device_id, thisDeviceId), snapshot.remote_path].filter(Boolean).join(", ")}
              </div>
            </div>
          </div>
        ))
      )}
    </PageSection>
  );
}
