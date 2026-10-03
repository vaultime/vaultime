// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useMemo, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Laptop } from "lucide-react";
import { Notice, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useLibrary } from "@/features/library/library-context";
import { GameLinkPicker } from "@/features/library/components/GameLinkPicker";
import { mergeChangesSomething, mergeDone, mergeHeadline, mergeNotes, sourceName } from "@/lib/merge";
import * as api from "@/lib/tauri";
import { formatHoursMinutes, formatLongDate } from "@/lib/time";
import type { MergePreview } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { plural } from "@/lib/words";

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

function MergeDialog({
  preview,
  onClose,
  onMerged,
}: {
  preview: MergePreview;
  onClose: () => void;
  onMerged: (message: string) => void;
}) {
  const { games } = useLibrary();
  const ours = useMemo(() => games.filter((game) => game.origin_device_id === null), [games]);
  const [choices, setChoices] = useState<Record<string, string | null>>(() =>
    Object.fromEntries(preview.games.map((game) => [game.game_id, game.suggested_game_id])),
  );
  const [merging, setMerging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // The player may vouch that new keys claiming the PC are its own. The
  // switch changes once the preview is read again with them, so what the
  // dialog shows is always what a merge does. Only the newest read counts.
  const [trustNewKeys, setTrustNewKeys] = useState(false);
  const [shown, setShown] = useState(preview);
  const [reading, setReading] = useState(false);
  const reads = useRef(0);
  const notes = mergeNotes(shown);
  const nothing = !mergeChangesSomething(shown);

  async function changeTrust(next: boolean) {
    const read = ++reads.current;
    setReading(true);
    setError(null);
    try {
      const again = await api.previewMerge(preview.backup_path, next);
      if (read !== reads.current) return;
      setShown(again);
      setTrustNewKeys(next);
    } catch (trustError) {
      if (read === reads.current) setError(describeError(trustError));
    } finally {
      if (read === reads.current) setReading(false);
    }
  }

  async function merge() {
    setMerging(true);
    setError(null);
    try {
      const summary = await api.mergeBackup(
        preview.backup_path,
        Object.entries(choices).map(([game_id, linked_game_id]) => ({ game_id, linked_game_id })),
        trustNewKeys,
      );
      onMerged(
        summary.safety_backup_path
          ? `${mergeDone(summary)} A backup from just before is in ${summary.safety_backup_path}.`
          : mergeDone(summary),
      );
    } catch (mergeError) {
      setError(describeError(mergeError));
    } finally {
      setMerging(false);
    }
  }

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Merge from {sourceName(preview)}</DialogTitle>
          <DialogDescription>{mergeHeadline(shown)}</DialogDescription>
        </DialogHeader>
        {notes.length > 0 && (
          <ul className="flex flex-col gap-1.5 text-[13px] leading-relaxed text-soft">
            {notes.map((note) => (
              <li key={note}>{note}</li>
            ))}
          </ul>
        )}
        {preview.unknown_keys && (
          <label htmlFor="trust-new-keys" className="flex items-start justify-between gap-4 border-t border-rule pt-3">
            <span className="min-w-0">
              <span className="block text-[13px] text-text">This backup comes from my {sourceName(preview)}</span>
              <span className="mt-1 block text-[12px] leading-relaxed text-faint">
                Only if you saved it there. Its key may be new here because its key file was lost or because its
                sessions came through another PC before. That key then vouches for its sessions here, also for
                those that came in Suspicious before.
              </span>
            </span>
            <Switch
              id="trust-new-keys"
              checked={trustNewKeys}
              disabled={reading}
              onCheckedChange={(next) => void changeTrust(next)}
            />
          </label>
        )}
        {preview.games.length > 0 && (
          <div className="flex max-h-72 flex-col overflow-y-auto border-t border-rule">
            <p className="label-caps pt-3 pb-1">Games that come along</p>
            {preview.games.map((game) => (
              <div key={game.game_id} className="flex items-center justify-between gap-4 border-b border-rule py-2.5">
                <div className="min-w-0">
                  <p className="truncate text-[14px] text-text">{game.title}</p>
                  <p className="text-[12px] text-faint tabular-nums">
                    {plural(game.sessions, "session")}, {formatHoursMinutes(game.runtime_ms)}
                  </p>
                </div>
                <GameLinkPicker
                  games={ours}
                  value={choices[game.game_id] ?? null}
                  suggested={game.suggested_game_id}
                  suggestedBecause={game.suggested_because}
                  onChange={(linked) => setChoices((current) => ({ ...current, [game.game_id]: linked }))}
                />
              </div>
            ))}
          </div>
        )}
        {!nothing && (
          <p className="text-[12px] leading-relaxed text-faint">
            Vaultime saves a backup of this PC first, so a restore can undo the merge.
          </p>
        )}
        {error && <p className="text-[13px] text-amber">{error}</p>}
        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            {nothing ? "Close" : "Cancel"}
          </Button>
          {!nothing && (
            <Button disabled={merging || reading} onClick={() => void merge()}>
              {merging ? "Merging…" : "Merge"}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
