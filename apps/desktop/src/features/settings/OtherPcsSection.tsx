// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { CloudDownload, Laptop } from "lucide-react";
import { Notice, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { useCloudSession } from "@/features/cloud/cloud-context";
import { useLibrary } from "@/features/library/library-context";
import * as api from "@/lib/tauri";
import { formatLongDate } from "@/lib/time";
import type { AutoMerge, CloudPc, GameChoice, MergePreview, MergeSummary } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { plural } from "@/lib/words";
import { MergeDialog } from "./MergeDialog";

/** A merge on its way: its preview and where it reads again and merges. */
interface PendingMerge {
  preview: MergePreview;
  readAgain: (trustNewKeys: boolean) => Promise<MergePreview>;
  mergeWith: (choices: GameChoice[], trustNewKeys: boolean) => Promise<MergeSummary>;
  /** What to do when the dialog closes without a merge. */
  dismiss?: () => void;
  fromCloud?: boolean;
}

const WAITING: Record<Exclude<AutoMerge["outcome"], "merged">, string> = {
  new_key: "Its newest backup brings a key this PC does not know yet, so it waits for you.",
  failing: "Its newest backup has sessions that fail their check, so it waits for you.",
};

/** What the cloud row of a PC says about it. */
function cloudHint(pc: CloudPc, waiting: AutoMerge | undefined): string {
  const backup = `Backup of ${formatLongDate(pc.backup_created_at)}, ${plural(pc.sessions_count, "session")}.`;
  if (waiting && waiting.outcome !== "merged") return `${backup} ${WAITING[waiting.outcome]}`;
  if (!pc.merged_before) return `${backup} After the first merge, newer backups come in by themselves.`;
  if (pc.newer) return `${backup} It comes in by itself within the hour.`;
  return `${backup} Merged.`;
}

/** Other PCs whose sessions are here, and merging in another one's backup, from a folder or the cloud. */
export function OtherPcsSection({ onError }: { onError: (message: string | null) => void }) {
  const { devices, thisDeviceId, sessions, refresh } = useLibrary();
  const cloud = useCloudSession();
  const [pending, setPending] = useState<PendingMerge | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [cloudPcs, setCloudPcs] = useState<CloudPc[] | null>(null);
  const [cloudError, setCloudError] = useState<string | null>(null);
  const cloudReady = cloud.enabled && cloud.session !== null && cloud.backupKeyReady;

  const others = useMemo(() => {
    const counts = new Map<string, number>();
    for (const session of sessions) counts.set(session.device_id, (counts.get(session.device_id) ?? 0) + 1);
    return devices
      .filter((device) => device.id !== thisDeviceId && (counts.get(device.id) ?? 0) > 0)
      .map((device) => ({ device, sessions: counts.get(device.id) ?? 0 }));
  }, [devices, thisDeviceId, sessions]);

  // Read again whenever the sessions change, as a merge changes what is due.
  const { listCloudPcs } = cloud;
  useEffect(() => {
    if (!cloudReady) return;
    let cancelled = false;
    listCloudPcs()
      .then((pcs) => {
        if (cancelled) return;
        setCloudPcs(pcs);
        setCloudError(null);
      })
      .catch((error) => {
        if (!cancelled) setCloudError(describeError(error));
      });
    return () => {
      cancelled = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- the provider makes the function anew on every render
  }, [cloudReady, sessions]);

  async function pickBackup() {
    onError(null);
    setMessage(null);
    const selected = await openFileDialog({ multiple: false, directory: true, title: "Pick a backup of another PC" });
    if (typeof selected !== "string") return;
    setBusy("file");
    try {
      setPending({
        preview: await api.previewMerge(selected),
        readAgain: (trustNewKeys) => api.previewMerge(selected, trustNewKeys),
        mergeWith: (choices, trustNewKeys) => api.mergeBackup(selected, choices, trustNewKeys),
      });
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusy(null);
    }
  }

  async function mergeFromCloud(pc: CloudPc) {
    onError(null);
    setMessage(null);
    setBusy(pc.backup_id);
    try {
      setPending({
        preview: await cloud.previewCloudMerge(pc.backup_id, false),
        readAgain: (trustNewKeys) => cloud.previewCloudMerge(pc.backup_id, trustNewKeys),
        mergeWith: (choices, trustNewKeys) => cloud.mergeCloudBackup(pc.backup_id, choices, trustNewKeys),
        dismiss: () => void cloud.discardCloudMerge(pc.backup_id).catch(() => {}),
        fromCloud: true,
      });
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusy(null);
    }
  }

  return (
    <PageSection
      title="Other PCs"
      description="Bring in the sessions of another PC from one of its backups. They come in as that PC recorded them, and this PC's ledger vouches only for those that pass their check with the keys it trusts for that PC, or for a trusted PC that brought them. The first time, it takes that PC's ledger at its word, from the cloud only with the keys that PC registered there. Only the PC that recorded a session can correct it."
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
      {cloudReady &&
        cloudPcs?.map((pc) => {
          const waiting = cloud.waitingMerges.find((merge) => merge.device_id === pc.device_id);
          const offer = !pc.merged_before || waiting !== undefined || pc.newer;
          return (
            <PageRow key={pc.device_id} label={`${pc.name ?? "Another PC"} in the cloud`} hint={cloudHint(pc, waiting)}>
              {offer && (
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busy !== null}
                  onClick={() => void mergeFromCloud(pc)}
                >
                  <CloudDownload className="size-3.5" />
                  {busy === pc.backup_id ? "Downloading…" : pc.merged_before && !waiting ? "Merge now" : "Merge"}
                </Button>
              )}
            </PageRow>
          );
        })}
      {cloudReady && cloudError && <Notice tone="warning">The cloud backups of other PCs could not be read: {cloudError}</Notice>}
      <PageRow
        label="Merge a backup of another PC"
        hint="Pick a backup folder of the other PC, saved there under Settings, Local backups. You see what comes in before anything changes."
      >
        <Button variant="outline" size="sm" disabled={busy !== null} onClick={() => void pickBackup()}>
          <Laptop className="size-3.5" />
          {busy === "file" ? "Reading…" : "Pick a backup"}
        </Button>
      </PageRow>
      {message && <Notice>{message}</Notice>}
      {pending && (
        <MergeDialog
          preview={pending.preview}
          readAgain={pending.readAgain}
          mergeWith={pending.mergeWith}
          fromCloud={pending.fromCloud}
          onClose={() => {
            pending.dismiss?.();
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
