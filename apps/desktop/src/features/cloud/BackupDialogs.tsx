// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { ArchiveRestore, Loader2, Trash2 } from "lucide-react";
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
import { formatSessionStart } from "@/lib/time";
import type { CloudBackupRecord } from "@/lib/types";
import { plural, whichPc } from "@/lib/words";
import { backupSize, backupSource, formatTimestamp } from "./backups";

/** One backup on the server, with buttons to restore or delete it. */
export function BackupRow({
  backup,
  thisDeviceId,
  canRestore,
  busy,
  onRestore,
  onDelete,
}: {
  backup: CloudBackupRecord;
  thisDeviceId: string | null;
  /** The backups are unlocked on this PC. */
  canRestore: boolean;
  busy: boolean;
  onRestore: () => void;
  onDelete: () => void;
}) {
  const madeOn = whichPc(backupSource(backup), thisDeviceId);
  return (
    <div className="flex flex-wrap items-baseline gap-x-5 gap-y-2 border-b border-rule py-3.5 last:border-b-0">
      <span className="w-[20ch] shrink-0 font-mono text-[13px] text-faint">{formatSessionStart(backup.uploaded_at)}</span>
      <div className="min-w-0 flex-1">
        <div className="text-[15px]">{backup.label ?? "Backup"}</div>
        <div className="mt-0.5 text-[13px] text-faint">
          {[
            backup.metadata_json && plural(backup.metadata_json.games_count, "game"),
            backup.metadata_json && plural(backup.metadata_json.sessions_count, "session"),
            backupSize(backup),
            madeOn && `made on ${madeOn}`,
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
          onClick={onRestore}
          disabled={busy || backup.status !== "complete" || !canRestore}
        >
          <ArchiveRestore className="size-3.5" />
          Restore
        </Button>
        <Button variant="ghost" size="sm" aria-label={`Delete ${backup.label ?? "backup"}`} onClick={onDelete} disabled={busy}>
          <Trash2 className="size-3.5" />
        </Button>
      </div>
    </div>
  );
}

/** Asks before a backup from the server replaces this PC's history. */
export function RestoreBackupDialog({
  open,
  onOpenChange,
  target,
  takesOver,
  busy,
  onRestore,
  onCancel,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  target: CloudBackupRecord | null;
  /** This PC has no history yet and the backup names another PC, which this one continues as if its ledger is sound. */
  takesOver: boolean;
  busy: boolean;
  onRestore: () => void;
  /** Closes the dialog and keeps the backup in it while it fades. */
  onCancel: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Restore this backup?</DialogTitle>
          <DialogDescription>It replaces the library, sessions, event logs and covers on this PC.</DialogDescription>
        </DialogHeader>

        {target && (
          <div className="grid gap-3">
            <p className="text-[15px]">
              {target.label ?? "Backup"}, uploaded {formatTimestamp(target.uploaded_at)}
            </p>
            <p className="font-mono text-sm text-soft">
              {target.metadata_json
                ? `${plural(target.metadata_json.games_count, "game")}, ${plural(target.metadata_json.sessions_count, "session")}`
                : "Contents unknown"}
            </p>
            {takesOver && (
              <p className="text-[15px]">
                This PC has no history of its own yet, so it continues as the PC that made this backup, as long as
                the backup&apos;s ledger is sound.
              </p>
            )}
            <p className="text-[13px] leading-relaxed text-faint">
              The download is checked against its checksum first. Close running games before you restore. Vaultime
              asks for a restart afterwards so tracking picks up cleanly.
            </p>
          </div>
        )}

        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button type="button" variant="destructive" onClick={onRestore} disabled={busy || !target}>
            {busy ? <Loader2 className="size-4 animate-spin" /> : <ArchiveRestore className="size-4" />}
            {busy ? "Restoring" : "Replace with this backup"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** Asks before a backup is removed from the server. */
export function DeleteBackupDialog({
  open,
  onOpenChange,
  target,
  busy,
  onDelete,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  target: CloudBackupRecord | null;
  busy: boolean;
  onDelete: () => void;
}) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete this backup?</DialogTitle>
          <DialogDescription>
            {target?.label ?? "The backup"}, {target ? backupSize(target) : ""}, is removed from the server. Nothing on
            this PC changes.
          </DialogDescription>
        </DialogHeader>

        <DialogFooter>
          <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="button" variant="destructive" onClick={onDelete} disabled={busy || !target}>
            {busy ? <Loader2 className="size-4 animate-spin" /> : <Trash2 className="size-4" />}
            {busy ? "Deleting" : "Delete backup"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
