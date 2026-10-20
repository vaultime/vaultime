// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { GameLinkPicker } from "@/features/library/components/GameLinkPicker";
import { useLibrary } from "@/features/library/library-context";
import { launcherName, whereFrom } from "@/lib/links";
import { mergeChangesSomething, mergeDone, mergeHeadline, mergeNotes, sourceName } from "@/lib/merge";
import { formatHoursMinutes } from "@/lib/time";
import type { GameChoice, MergePreview, MergeSummary } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { plural } from "@/lib/words";

/** What a merge of another PC's backup brings, which game each of its games counts as, and the merge. */
export function MergeDialog({
  preview,
  readAgain,
  mergeWith,
  fromCloud = false,
  onClose,
  onMerged,
}: {
  preview: MergePreview;
  /** Reads the preview again, with or without the player vouching for new keys. */
  readAgain: (trustNewKeys: boolean) => Promise<MergePreview>;
  mergeWith: (choices: GameChoice[], trustNewKeys: boolean) => Promise<MergeSummary>;
  /** Whether the backup comes from the cloud, where a first merge counts only the keys a PC registered. */
  fromCloud?: boolean;
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
  const notes = mergeNotes(shown, { fromCloud });
  const nothing = !mergeChangesSomething(shown);

  async function changeTrust(next: boolean) {
    const read = ++reads.current;
    setReading(true);
    setError(null);
    try {
      const again = await readAgain(next);
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
      const summary = await mergeWith(
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
          <div className="border-t border-rule">
            {/* Room at the sides, so the scrolling list does not cut off the
                pickers' focus ring. */}
            <div className="-mx-1 flex max-h-72 flex-col overflow-y-auto px-1">
              <p className="label-caps pt-3 pb-1">Games that come along</p>
              {preview.games.map((game) => (
                <div key={game.game_id} className="flex items-center justify-between gap-4 border-b border-rule py-2.5">
                  <div className="min-w-0">
                    <p className="truncate text-[14px] text-text">{game.title}</p>
                    <p className="text-[12px] text-faint tabular-nums">
                      {plural(game.sessions, "session")}, {formatHoursMinutes(game.runtime_ms)}
                      {launcherName(game.launcher_source) && `, ${whereFrom(game.launcher_source)}`}
                    </p>
                  </div>
                  {game.counts_as ? (
                    // It follows the game it counts as there, whatever that one counts as here.
                    <span className="max-w-64 shrink-0 truncate text-right text-[13px] text-faint">
                      Same game as {game.counts_as_title ?? "another game"} there
                    </span>
                  ) : (
                    <GameLinkPicker
                      games={ours}
                      value={choices[game.game_id] ?? null}
                      suggested={game.suggested_game_id}
                      suggestedBecause={game.suggested_because}
                      onChange={(linked) => setChoices((current) => ({ ...current, [game.game_id]: linked }))}
                    />
                  )}
                </div>
              ))}
            </div>
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
