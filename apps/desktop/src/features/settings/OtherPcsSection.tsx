// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useMemo, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Laptop } from "lucide-react";
import { Notice, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { useLibrary } from "@/features/library/library-context";
import * as api from "@/lib/tauri";
import { formatLongDate } from "@/lib/time";
import type { MergePreview } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { plural } from "@/lib/words";
import { MergeDialog } from "./MergeDialog";

/** Other PCs whose sessions are here, and merging in another one's backup. */
export function OtherPcsSection({ onError }: { onError: (message: string | null) => void }) {
  const { devices, thisDeviceId, sessions, refresh } = useLibrary();
  const [preview, setPreview] = useState<MergePreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const others = useMemo(() => {
    const counts = new Map<string, number>();
    for (const session of sessions) counts.set(session.device_id, (counts.get(session.device_id) ?? 0) + 1);
    return devices
      .filter((device) => device.id !== thisDeviceId && (counts.get(device.id) ?? 0) > 0)
      .map((device) => ({ device, sessions: counts.get(device.id) ?? 0 }));
  }, [devices, thisDeviceId, sessions]);

  async function pickBackup() {
    onError(null);
    setMessage(null);
    const selected = await openFileDialog({ multiple: false, directory: true, title: "Pick a backup of another PC" });
    if (typeof selected !== "string") return;
    setBusy(true);
    try {
      setPreview(await api.previewMerge(selected));
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <PageSection
      title="Other PCs"
      description="Bring in the sessions of another PC from one of its backups. They come in as that PC recorded them, and this PC's ledger vouches only for those that pass their check with the keys it trusts for that PC, or for a trusted PC that brought them. The first time, it takes that PC's ledger at its word. Only the PC that recorded a session can correct it."
    >
      {others.map(({ device, sessions: count }) => (
        <PageRow
          key={device.id}
          label={device.name ?? device.id}
          hint={device.merged_at ? `Merged ${formatLongDate(device.merged_at)}.` : "Came with a restored backup."}
        >
          <span className="text-soft tabular-nums">{plural(count, "session")}</span>
        </PageRow>
      ))}
      <PageRow
        label="Merge a backup of another PC"
        hint="Pick a backup folder of the other PC, saved there under Settings, Local backups. You see what comes in before anything changes."
      >
        <Button variant="outline" size="sm" disabled={busy} onClick={() => void pickBackup()}>
          <Laptop className="size-3.5" />
          {busy ? "Reading…" : "Pick a backup"}
        </Button>
      </PageRow>
      {message && <Notice>{message}</Notice>}
      {preview && (
        <MergeDialog
          preview={preview}
          onClose={() => setPreview(null)}
          onMerged={(done) => {
            setPreview(null);
            setMessage(done);
            refresh().catch(() => {});
          }}
        />
      )}
    </PageSection>
  );
}
