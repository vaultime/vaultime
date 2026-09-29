// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, type FormEvent } from "react";
import { Notice } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { SESSION_NOTE_MAX_CHARS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import { formatHoursMinutes, fromLocalInput, parseVaultimeDate, toLocalInput } from "@/lib/time";
import type { Session } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";

type Mode = "trim" | "discard";

/**
 * Corrects a finished session: counts it only up to a time, or takes all its
 * time out. The session keeps its old times and the reason in its history.
 */
export function CorrectSessionDialog({
  session,
  gameTitle,
  open,
  onOpenChange,
  onCorrected,
}: {
  session: Session;
  gameTitle?: string;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onCorrected: () => void;
}) {
  const start = parseVaultimeDate(session.started_at_wall);
  const end = session.ended_at_wall ? parseVaultimeDate(session.ended_at_wall) : start;
  const [mode, setMode] = useState<Mode>("trim");
  const [endInput, setEndInput] = useState(toLocalInput(end));
  const [reason, setReason] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // The field drops seconds, so an untouched field keeps the exact end.
  const newEnd = endInput === toLocalInput(end) ? end : fromLocalInput(endInput);
  // The same rule as the core: the cut comes out of idle time first.
  const removed =
    mode === "discard"
      ? session.runtime_ms
      : newEnd
        ? Math.min(Math.max(0, end.getTime() - newEnd.getTime()), session.runtime_ms)
        : 0;
  const idleCut = Math.min(removed, session.idle_ms);
  const counts = session.runtime_ms - removed;
  const activeAfter = session.active_ms - Math.min(removed - idleCut, session.active_ms);
  const endValid = mode === "discard" || (newEnd !== null && newEnd >= start && newEnd <= end);

  async function save(event: FormEvent) {
    event.preventDefault();
    setSaving(true);
    setError(null);
    try {
      if (mode === "discard") {
        await api.discardSession(session.id, reason);
      } else if (newEnd) {
        await api.trimSession(session.id, newEnd.toISOString(), reason);
      }
      onCorrected();
      onOpenChange(false);
    } catch (saveError) {
      setError(describeError(saveError));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <form className="grid gap-5" onSubmit={(event) => void save(event)}>
          <DialogHeader>
            <DialogTitle>Correct this session</DialogTitle>
            <DialogDescription>
              {gameTitle ? `${gameTitle}, ` : ""}
              {formatHoursMinutes(session.runtime_ms)} in all. The old times and your reason stay in its history
              {session.integrity_status === "suspicious" ? ", and it stays labeled Suspicious." : ", and it is labeled Edited."}
            </DialogDescription>
          </DialogHeader>

          <div role="radiogroup" aria-label="How to correct it" className="grid gap-2">
            {(
              [
                ["trim", "Count it only until a time", "For a game left running after you stopped playing."],
                ["discard", "Take out all its time", "For a session that was no play at all."],
              ] as const
            ).map(([value, label, hint]) => (
              <label
                key={value}
                className={cn(
                  "flex cursor-pointer items-start gap-3 rounded-lg border px-3.5 py-3",
                  mode === value ? "border-violet/60 bg-raised" : "border-hairline",
                )}
              >
                <input
                  type="radio"
                  name="mode"
                  value={value}
                  checked={mode === value}
                  onChange={() => setMode(value)}
                  className="mt-1 accent-violet"
                />
                <span>
                  <span className="block text-sm text-text">{label}</span>
                  <span className="block text-[13px] text-faint">{hint}</span>
                </span>
              </label>
            ))}
          </div>

          {mode === "trim" && (
            <Field id="correct-end" label="Stopped playing at">
              <Input
                id="correct-end"
                type="datetime-local"
                value={endInput}
                min={toLocalInput(start)}
                max={toLocalInput(end)}
                onChange={(event) => setEndInput(event.target.value)}
              />
            </Field>
          )}

          <Field id="correct-reason" label="Why">
            <Input
              id="correct-reason"
              value={reason}
              maxLength={SESSION_NOTE_MAX_CHARS}
              placeholder={mode === "trim" ? "Left the game running overnight" : "Only the launcher was open"}
              onChange={(event) => setReason(event.target.value)}
            />
          </Field>

          <p className="text-sm text-soft">
            {counts === session.runtime_ms
              ? "Nothing changes yet."
              : `Counts ${formatHoursMinutes(counts)} instead of ${formatHoursMinutes(session.runtime_ms)}, ${formatHoursMinutes(activeAfter)} of it active.`}
          </p>

          {error && <Notice tone="warning">{error}</Notice>}

          <DialogFooter>
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              Cancel
            </Button>
            <Button
              type="submit"
              disabled={saving || !reason.trim() || !endValid || counts === session.runtime_ms}
            >
              {saving ? "Saving" : "Correct"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
