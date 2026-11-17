// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { formatLongDate } from "@/lib/time";
import type { AutoMerge, CloudPc, MergePreview, MergeSummary } from "@/lib/types";
import { capitalize, plural } from "@/lib/words";

/** Why an automatic merge left a PC's newest backup for the player. */
const WAITING: Record<Exclude<AutoMerge["outcome"], "merged">, string> = {
  new_key: "Its newest backup brings a key this PC does not know yet, so it waits for you.",
  failing: "Its newest backup has sessions that fail their check, so it waits for you.",
  error: "Its newest backup did not come in by itself yet. If that lasts, merge it to see why.",
};

/** What the row of another PC's newest backup, in the cloud or the sync folder, says about it. */
export function backupHint(
  pc: Pick<CloudPc, "backup_created_at" | "sessions_count" | "merged_before" | "newer">,
  waiting?: AutoMerge,
): string {
  const backup = `Backup of ${formatLongDate(pc.backup_created_at)}, ${plural(pc.sessions_count, "session")}.`;
  if (waiting && waiting.outcome !== "merged") return `${backup} ${WAITING[waiting.outcome]}`;
  if (!pc.merged_before) return `${backup} After the first merge, newer backups come in by themselves.`;
  if (pc.newer) return `${backup} It comes in by itself within the hour.`;
  return `${backup} Merged.`;
}

/** The PC a backup comes from, by the name it gave itself. */
export function sourceName(preview: Pick<MergePreview, "device_name" | "device_id">): string {
  return preview.device_name ?? preview.device_id;
}

/** Whether a merge would change anything. */
export function mergeChangesSomething(preview: MergePreview): boolean {
  return preview.new_sessions + preview.grown_sessions + preview.vouched_now > 0;
}

/** What a merge brings in, in a sentence or two. */
export function mergeHeadline(preview: MergePreview): string {
  const from = sourceName(preview);
  const when = formatLongDate(preview.backup_created_at);
  const parts = [];
  if (preview.new_sessions > 0) parts.push(`${plural(preview.new_sessions, "session")} that are not here yet`);
  if (preview.grown_sessions > 0) {
    parts.push(`${plural(preview.grown_sessions, "session")} that went on there`);
  }
  const vouched =
    preview.vouched_now > 0
      ? `${plural(preview.vouched_now, "session")} here ${preview.vouched_now === 1 ? "passes its" : "pass their"} check now`
      : null;
  if (parts.length === 0) {
    return vouched ? `With ${from}'s backup of ${when}, ${vouched}.` : `Nothing new to merge from ${from}'s backup of ${when}.`;
  }
  const coming = `From ${from}'s backup of ${when} come ${parts.join(" and ")}.`;
  return vouched ? `${coming} ${capitalize(vouched)}.` : coming;
}

/** What stays as it is, and what comes in without this PC vouching for it. */
export function mergeNotes(preview: MergePreview, { fromCloud = false } = {}): string[] {
  const from = sourceName(preview);
  const notes = [];
  if (preview.first_merge) {
    notes.push(
      fromCloud
        ? `Vaultime has not seen ${from} before, so it takes its ledger at its word for the keys ${from} registered with the cloud.`
        : `Vaultime has not seen ${from} before, so it takes its ledger at its word, as it does for any new PC.`,
    );
  }
  if (preview.failing > 0) {
    notes.push(
      `${plural(preview.failing, "session")} ${preview.failing === 1 ? "does" : "do"} not pass ${preview.failing === 1 ? "its" : "their"} check here, so ${preview.failing === 1 ? "it comes" : "they come"} in Suspicious.`,
    );
  }
  if (preview.own_failing > 0) {
    const one = preview.own_failing === 1;
    notes.push(
      `${plural(preview.own_failing, "session")} this PC played before it was set up again ${one ? "fails its" : "fail their"} check there, so ${one ? "it stays" : "they stay"} out.`,
    );
  }
  if (preview.unknown_keys) {
    notes.push(
      fromCloud && preview.first_merge
        ? `${from} signs with a key it never registered with the cloud, so that key vouches for nothing here unless you say so below.`
        : `${from} signs with a key this PC does not trust for it, so that key vouches for nothing here unless you say so below.`,
    );
  }
  if (preview.overlapping_manual > 0) {
    const one = preview.overlapping_manual === 1;
    notes.push(
      `${plural(preview.overlapping_manual, "session")} you added by hand here ${one ? "overlaps" : "overlap"} sessions that come in. If ${one ? "it was" : "they were"} the same play, take ${one ? "it" : "them"} out after the merge.`,
    );
  }
  const changed = preview.conflicts.filter((conflict) => conflict.reason === "changed_on_both").length;
  if (changed > 0) {
    notes.push(`${plural(changed, "session")} went on differently on both PCs and ${changed === 1 ? "stays" : "stay"} as here.`);
  }
  const failingUpdates = preview.conflicts.length - changed;
  if (failingUpdates > 0) {
    notes.push(
      `${plural(failingUpdates, "session")} went on there but ${failingUpdates === 1 ? "fails" : "fail"} its check, so the copy here stays.`,
    );
  }
  if (preview.running_there > 0) {
    notes.push(`${plural(preview.running_there, "session")} still ran there and ${preview.running_there === 1 ? "comes" : "come"} with the next merge.`);
  }
  if (preview.removed_here > 0) {
    notes.push(`${plural(preview.removed_here, "session")} you removed here ${preview.removed_here === 1 ? "stays" : "stay"} away.`);
  }
  if (preview.already_here > 0) {
    notes.push(`${plural(preview.already_here, "session")} ${preview.already_here === 1 ? "is" : "are"} here already.`);
  }
  return notes;
}

/** What a merge did. */
export function mergeDone(summary: MergeSummary): string {
  const from = summary.device_name ?? summary.device_id;
  const sessions = summary.sessions_added + summary.sessions_grown;
  const vouched =
    summary.sessions_vouched > 0
      ? `This PC vouches for ${plural(summary.sessions_vouched, "more session")} of ${from} now.`
      : null;
  if (sessions === 0) return vouched ?? `Nothing new came from ${from}.`;
  const merged = `Merged ${plural(sessions, "session")} from ${from}.`;
  return vouched ? `${merged} ${vouched}` : merged;
}
