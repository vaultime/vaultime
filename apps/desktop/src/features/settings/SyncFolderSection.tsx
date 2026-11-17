// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { ArchiveRestore, FolderSync, GitMerge } from "lucide-react";
import { Notice, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { useLibrary } from "@/features/library/library-context";
import { MergeDialog } from "@/features/settings/MergeDialog";
import { RestorePreview } from "@/features/settings/RestorePreview";
import { useSettings } from "@/features/settings/settings-context";
import { useSyncFolder } from "@/features/settings/sync-folder-context";
import { useFinishRestore } from "@/features/settings/use-finish-restore";
import { SETTING_KEYS, SYNC_BACKUP_KEEP } from "@/lib/constants";
import { backupHint } from "@/lib/merge";
import * as api from "@/lib/tauri";
import { formatLongDate } from "@/lib/time";
import type { AutoMerge, FolderPc, LocalBackupSummary, MergePreview } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { numberWords } from "@/lib/words";

/** What the row of a PC in the sync folder says about it. */
function pcHint(pc: FolderPc, waiting: AutoMerge | undefined): string {
  if (pc.this_pc) return `Backup of ${formatLongDate(pc.backup_created_at)}. A new one comes in once a day.`;
  return backupHint(pc, waiting);
}

/** The sync folder: where it is, the backups of this PC and the others in it, their merges and restores. */
export function SyncFolderSection({
  onError,
  onRestartRequired,
}: {
  onError: (message: string | null) => void;
  onRestartRequired: () => void;
}) {
  const { sessions, refresh } = useLibrary();
  const { values } = useSettings();
  const finishRestore = useFinishRestore();
  const { waiting } = useSyncFolder();
  const autoBackup = values[SETTING_KEYS.autoBackup] !== "false";
  const [folder, setFolder] = useState<string | null>(null);
  const [pcs, setPcs] = useState<FolderPc[]>([]);
  // What is being read or changed: the folder, or a PC's merge or restore.
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [pending, setPending] = useState<{ deviceId: string; preview: MergePreview } | null>(null);
  const [restoring, setRestoring] = useState<{ deviceId: string; summary: LocalBackupSummary } | null>(null);
  // Taking over the backup's PC is right when this PC is that one set up again.
  const [takeOver, setTakeOver] = useState(true);

  // Read again whenever the sessions change, as a merge changes what is due.
  useEffect(() => {
    let cancelled = false;
    Promise.all([api.getSyncFolder(), api.listSyncFolderPcs()])
      .then(([nextFolder, nextPcs]) => {
        if (cancelled) return;
        setFolder(nextFolder);
        setPcs(nextPcs);
      })
      .catch((error) => {
        if (!cancelled) onError(describeError(error));
      });
    return () => {
      cancelled = true;
    };
  }, [sessions, onError]);

  async function run(key: string, task: () => Promise<void>) {
    try {
      setBusy(key);
      onError(null);
      setMessage(null);
      await task();
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusy(null);
    }
  }

  const choose = () =>
    run("folder", async () => {
      const picked = await openFileDialog({ multiple: false, directory: true, title: "Choose the sync folder" });
      if (typeof picked !== "string") return;
      setFolder(await api.setSyncFolder(picked));
      const found = await api.listSyncFolderPcs();
      setPcs(found);
      // A PC that played nothing yet copies nothing, so a new one can restore first.
      setMessage(
        found.some((pc) => pc.this_pc)
          ? "A backup of this PC is in the folder now. Choose the same folder on your other PCs."
          : "Once you play, a backup of this PC goes into the folder. Choose the same folder on your other PCs.",
      );
    });

  const stop = () =>
    run("folder", async () => {
      await api.setSyncFolder(null);
      setFolder(null);
      setPcs([]);
      setMessage("The backups stay in the folder. New ones stay on this PC.");
    });

  const merge = (pc: FolderPc) =>
    run(`merge:${pc.device_id}`, async () => {
      setRestoring(null);
      setPending({ deviceId: pc.device_id, preview: await api.previewSyncMerge(pc.device_id) });
    });

  const inspect = (pc: FolderPc) =>
    run(`restore:${pc.device_id}`, async () => {
      setRestoring({ deviceId: pc.device_id, summary: await api.inspectSyncBackup(pc.device_id) });
      setTakeOver(true);
    });

  const restore = () =>
    run("restore", async () => {
      if (!restoring) return;
      // Only a takeover the preview offered, as in the other restores.
      const offered = Boolean(restoring.summary.continues_as);
      const summary = await api.restoreSyncBackup(restoring.deviceId, takeOver && offered);
      setRestoring(null);
      if (summary.restart_required) onRestartRequired();
      setMessage(await finishRestore("Backup", summary));
    });

  return (
    <PageSection
      title="Sync folder"
      description={`Keep backups in a folder that a sync service or a NAS keeps the same on every PC, such as one in Dropbox, Google Drive, OneDrive or Syncthing. Each PC copies its automatic backups there, the newest ${numberWords(SYNC_BACKUP_KEEP)}, and the PCs that share the folder bring in each other's sessions: the first time through you, after that by themselves.`}
    >
      <PageRow
        label={
          <>
            Folder
            <span className="mt-1 block font-mono text-xs break-all text-soft">{folder ?? "Not set"}</span>
          </>
        }
        hint="Vaultime keeps a folder named Vaultime in it, with a folder for each PC. Choose the same folder on every PC."
      >
        <span className="flex gap-2">
          <Button variant="outline" size="sm" onClick={choose} disabled={busy !== null}>
            <FolderSync className="size-3.5" />
            {folder ? "Change" : "Choose"}
          </Button>
          {folder && (
            <Button variant="ghost" size="sm" onClick={stop} disabled={busy !== null}>
              Stop
            </Button>
          )}
        </span>
      </PageRow>
      {folder && !autoBackup && (
        <Notice tone="warning" className="mt-4">
          Automatic backups are off, so no new backup of this PC reaches the folder.
        </Notice>
      )}
      {pcs.map((pc) => {
        // Merged by hand since the last check, it waits no more.
        const waits = pc.newer
          ? waiting.find((merge) => merge.device_id === pc.device_id && merge.backup_id === pc.backup_id)
          : undefined;
        return (
          <PageRow
            key={pc.device_id}
            label={pc.this_pc ? `${pc.name ?? "This PC"}, this PC` : (pc.name ?? "Another PC")}
            hint={pcHint(pc, waits)}
          >
            <span className="flex gap-2">
              {!pc.this_pc && (!pc.merged_before || waits) && (
                <Button variant="outline" size="sm" disabled={busy !== null} onClick={() => void merge(pc)}>
                  <GitMerge className="size-3.5" />
                  {busy === `merge:${pc.device_id}` ? "Reading…" : "Merge"}
                </Button>
              )}
              <Button variant="ghost" size="sm" disabled={busy !== null} onClick={() => void inspect(pc)}>
                <ArchiveRestore className="size-3.5" />
                {busy === `restore:${pc.device_id}` ? "Reading…" : "Restore"}
              </Button>
            </span>
          </PageRow>
        );
      })}
      {folder && pcs.length === 0 && <p className="py-3 text-sm text-faint">No backups in the folder yet.</p>}
      {message && <Notice className="mt-5">{message}</Notice>}
      {restoring && (
        <RestorePreview
          id="sync-take-over"
          summary={restoring.summary}
          takeOver={takeOver}
          busy={busy !== null}
          onTakeOverChange={setTakeOver}
          onRestore={restore}
          onCancel={() => {
            void api.discardSyncMerge(restoring.deviceId).catch(() => {});
            setRestoring(null);
          }}
        />
      )}
      {pending && (
        <MergeDialog
          preview={pending.preview}
          readAgain={(trustNewKeys) => api.previewSyncMerge(pending.deviceId, trustNewKeys)}
          mergeWith={(choices, trustNewKeys) => api.mergeSyncBackup(pending.deviceId, choices, trustNewKeys)}
          onClose={() => {
            void api.discardSyncMerge(pending.deviceId).catch(() => {});
            setPending(null);
          }}
          onMerged={(done) => {
            setPending(null);
            setMessage(done);
            refresh().catch(() => {});
          }}
        />
      )}
    </PageSection>
  );
}
