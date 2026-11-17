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
import {
  HOUR_MS,
  MANUAL_SESSION_DEFAULT_AGO_MS,
  MANUAL_SESSION_MAX_HOURS,
  MINUTE_MS,
  MINUTES_PER_HOUR,
  SESSION_NOTE_MAX_CHARS,
  STEAM_LAUNCHER,
} from "@/lib/constants";
import * as api from "@/lib/tauri";
import { formatHoursMinutes, fromLocalInput, parseVaultimeDate, toLocalInput } from "@/lib/time";
import { describeError } from "@/lib/utils";

/** The whole minutes from `from` to `now` as hours and minutes, at most the longest session. */
function timedLength(from: string, now: number): { hours: string; minutes: string } {
  const elapsed = Math.floor((now - parseVaultimeDate(from).getTime()) / MINUTE_MS);
  const total = Math.min(Math.max(elapsed, 0), MANUAL_SESSION_MAX_HOURS * MINUTES_PER_HOUR);
  return { hours: String(Math.floor(total / MINUTES_PER_HOUR)), minutes: String(total % MINUTES_PER_HOUR) };
}

/** Adds play Vaultime did not see, like a session on a console or another PC. */
export function AddSessionDialog({
  gameId,
  gameTitle,
  steamGame,
  open,
  onOpenChange,
  onAdded,
  timedFrom,
  onDiscard,
}: {
  gameId: string;
  gameTitle: string;
  /** Offers to mark the play as counted by Steam too. */
  steamGame: boolean;
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdded: () => void;
  /** When the timer for play away started, which fills in the start and the length. */
  timedFrom?: string;
  /** Sets the timed play aside. */
  onDiscard?: () => void;
}) {
  // The dialog mounts when it opens, so this is the moment the player started adding.
  const [openedAt] = useState(() => Date.now());
  const [startInput, setStartInput] = useState(() =>
    toLocalInput(timedFrom ? parseVaultimeDate(timedFrom) : new Date(openedAt - MANUAL_SESSION_DEFAULT_AGO_MS)),
  );
  const [hours, setHours] = useState(() => (timedFrom ? timedLength(timedFrom, openedAt).hours : "1"));
  const [minutes, setMinutes] = useState(() => (timedFrom ? timedLength(timedFrom, openedAt).minutes : "0"));
  const [reason, setReason] = useState("");
  const [throughSteam, setThroughSteam] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const start = fromLocalInput(startInput);
  // The core takes whole milliseconds, and decimal hours need not give them.
  const runtimeMs = Math.round((Number(hours) || 0) * HOUR_MS + (Number(minutes) || 0) * MINUTE_MS);
  const endsInFuture = start !== null && start.getTime() + runtimeMs > openedAt;
  const valid = start !== null && runtimeMs > 0 && runtimeMs <= MANUAL_SESSION_MAX_HOURS * HOUR_MS && !endsInFuture;

  async function save(event: FormEvent) {
    event.preventDefault();
    if (!start) return;
    setSaving(true);
    setError(null);
    try {
      await api.addManualSession(
        gameId,
        start.toISOString(),
        runtimeMs,
        reason,
        steamGame && throughSteam ? STEAM_LAUNCHER : null,
      );
      onAdded();
      onOpenChange(false);
    } catch (saveError) {
      setError(describeError(saveError));
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <form className="grid gap-5" onSubmit={(event) => void save(event)}>
          <DialogHeader>
            <DialogTitle>{timedFrom ? "Save the timed play" : "Add a session"}</DialogTitle>
            <DialogDescription>
              {timedFrom
                ? `Play of ${gameTitle} away from this PC. Change the length if the timer ran on after you stopped playing. It counts as active time and is labeled Manual.`
                : `Play of ${gameTitle} that Vaultime did not see. It counts as active time and is labeled Manual.`}
            </DialogDescription>
          </DialogHeader>

          <Field id="manual-start" label="Started at">
            <Input
              id="manual-start"
              type="datetime-local"
              value={startInput}
              max={toLocalInput(new Date(openedAt))}
              onChange={(event) => setStartInput(event.target.value)}
            />
          </Field>

          <div className="grid grid-cols-2 gap-3">
            <Field id="manual-hours" label="Hours">
              <Input
                id="manual-hours"
                type="number"
                min={0}
                max={MANUAL_SESSION_MAX_HOURS}
                value={hours}
                onChange={(event) => setHours(event.target.value)}
              />
            </Field>
            <Field id="manual-minutes" label="Minutes">
              <Input
                id="manual-minutes"
                type="number"
                min={0}
                max={MINUTES_PER_HOUR - 1}
                value={minutes}
                onChange={(event) => setMinutes(event.target.value)}
              />
            </Field>
          </div>

          <Field id="manual-reason" label="Where, optional">
            <Input
              id="manual-reason"
              value={reason}
              maxLength={SESSION_NOTE_MAX_CHARS}
              placeholder="On the Steam Deck"
              onChange={(event) => setReason(event.target.value)}
            />
          </Field>

          {steamGame && (
            <label className="flex cursor-pointer items-start gap-3">
              <input
                type="checkbox"
                checked={throughSteam}
                onChange={(event) => setThroughSteam(event.target.checked)}
                className="mt-1 accent-violet"
              />
              <span>
                <span className="block text-sm text-text">Played through Steam</span>
                <span className="block text-[13px] text-faint">
                  On a Steam Deck or another PC with your Steam account. Steam counted this time too, so its playtime
                  import leaves it out.
                </span>
              </span>
            </label>
          )}

          <p className="text-sm text-soft">
            {endsInFuture
              ? "The session would end in the future."
              : runtimeMs > 0
                ? `Adds ${formatHoursMinutes(runtimeMs)} of active time.`
                : "Say how long you played."}
          </p>

          {error && <Notice tone="warning">{error}</Notice>}

          <DialogFooter>
            {onDiscard && (
              <Button type="button" variant="ghost" className="sm:mr-auto" onClick={onDiscard}>
                Discard
              </Button>
            )}
            <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
              {timedFrom ? "Keep timing" : "Cancel"}
            </Button>
            <Button type="submit" disabled={saving || !valid}>
              {saving ? "Adding" : timedFrom ? "Save session" : "Add session"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
