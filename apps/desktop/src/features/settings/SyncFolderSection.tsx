// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { FolderSync } from "lucide-react";
import { Notice, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { useLibrary } from "@/features/library/library-context";
import { useSettings } from "@/features/settings/settings-context";
import { SETTING_KEYS, SYNC_BACKUP_KEEP } from "@/lib/constants";
import { backupHint } from "@/lib/merge";
import * as api from "@/lib/tauri";
import { formatLongDate } from "@/lib/time";
import type { FolderPc } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { numberWords } from "@/lib/words";

/** What the row of a PC in the sync folder says about it. */
function pcHint(pc: FolderPc): string {
  if (pc.this_pc) return `Backup of ${formatLongDate(pc.backup_created_at)}. A new one comes in once a day and when Vaultime quits.`;
  return backupHint(pc);
}

/** The sync folder: where it is, and the backups of this PC and the others in it. */
export function SyncFolderSection({ onError }: { onError: (message: string | null) => void }) {
  const { sessions } = useLibrary();
  const { values } = useSettings();
  const autoBackup = values[SETTING_KEYS.autoBackup] !== "false";
  const [folder, setFolder] = useState<string | null>(null);
  const [pcs, setPcs] = useState<FolderPc[]>([]);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

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

  async function run(task: () => Promise<void>) {
    try {
      setBusy(true);
      onError(null);
      setMessage(null);
      await task();
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  const choose = () =>
    run(async () => {
      const picked = await openFileDialog({ multiple: false, directory: true, title: "Choose the sync folder" });
      if (typeof picked !== "string") return;
      setFolder(await api.setSyncFolder(picked));
      setPcs(await api.listSyncFolderPcs());
      setMessage("A backup of this PC is in the folder now. Choose the same folder on your other PCs.");
    });

  const stop = () =>
    run(async () => {
      await api.setSyncFolder(null);
      setFolder(null);
      setPcs([]);
      setMessage("The backups stay in the folder. New ones stay on this PC.");
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
          <Button variant="outline" size="sm" onClick={choose} disabled={busy}>
            <FolderSync className="size-3.5" />
            {folder ? "Change" : "Choose"}
          </Button>
          {folder && (
            <Button variant="ghost" size="sm" onClick={stop} disabled={busy}>
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
      {pcs.map((pc) => (
        <PageRow key={pc.device_id} label={pc.this_pc ? `${pc.name ?? "This PC"}, this PC` : (pc.name ?? "Another PC")} hint={pcHint(pc)} />
      ))}
      {folder && pcs.length === 0 && <p className="py-3 text-sm text-faint">No backups in the folder yet.</p>}
      {message && <Notice className="mt-5">{message}</Notice>}
    </PageSection>
  );
}
