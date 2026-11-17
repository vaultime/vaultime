// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { ArchiveRestore } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useLibrary } from "@/features/library/library-context";
import { TakeoverChoice } from "@/features/settings/TakeoverChoice";
import { continuesAs } from "@/lib/devices";
import { formatLongDate } from "@/lib/time";
import type { LocalBackupSummary } from "@/lib/types";
import { plural, whichPc } from "@/lib/words";

/** A backup about to be restored: what it holds, whether this PC takes over its PC, and the restore. */
export function RestorePreview({
  id,
  summary,
  takeOver,
  busy,
  onTakeOverChange,
  onRestore,
  onCancel,
}: {
  /** Id of the takeover switch, unique on the page. */
  id: string;
  summary: LocalBackupSummary;
  takeOver: boolean;
  busy: boolean;
  onTakeOverChange: (takeOver: boolean) => void;
  onRestore: () => void;
  onCancel: () => void;
}) {
  const { thisDeviceId } = useLibrary();
  const madeOn = whichPc(summary.source_device_id, thisDeviceId);
  return (
    <div className="mt-6 border-l-2 border-amber py-1 pl-5">
      <p className="text-[15px]">
        Backup from {formatLongDate(summary.created_at)}
        {madeOn && `, made on ${madeOn}`}.
      </p>
      <p className="mt-2 text-sm text-soft tabular-nums">
        {plural(summary.games_count, "game")}, {plural(summary.sessions_count, "session")},{" "}
        {plural(summary.asset_file_count, "image")}
      </p>
      {summary.continues_as && (
        <div className="mt-3">
          <TakeoverChoice
            id={id}
            takeover={summary.continues_as}
            checked={takeOver}
            disabled={busy}
            onCheckedChange={onTakeOverChange}
          >
            <p className="text-[15px] leading-relaxed">
              {continuesAs(summary.continues_as)}{" "}
              <span className="text-[13px] text-faint">
                This PC has no history of its own yet, so it takes over the backup&apos;s PC: its sessions count as
                this PC&apos;s own, and so do sessions it played after the backup that come in by merge, as long as
                that PC checked them only with this PC&apos;s keys.
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
        <Button variant="destructive" onClick={onRestore} disabled={busy}>
          <ArchiveRestore className="size-4" />
          Replace with this backup
        </Button>
        <Button variant="ghost" onClick={onCancel} disabled={busy}>
          Cancel
        </Button>
      </div>
    </div>
  );
}
