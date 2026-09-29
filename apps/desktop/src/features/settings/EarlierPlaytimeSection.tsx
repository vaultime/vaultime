// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import { History, Loader2, Trash2 } from "lucide-react";
import { Notice, PageRow, PageSection } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useLibrary } from "@/features/library/library-context";
import { MINUTE_MS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import { formatCalendarDay, formatHoursMinutes } from "@/lib/time";
import type { SteamPlaytimePreview } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { numberWords } from "@/lib/words";

const STEAM = "steam";

/** Imports the playtime Steam counted before Vaultime, with a look at it first. */
export function EarlierPlaytimeSection({ onError }: { onError: (message: string) => void }) {
  const { summaries, refresh } = useLibrary();
  const [open, setOpen] = useState(false);
  const [preview, setPreview] = useState<SteamPlaytimePreview | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);

  const imported = summaries.filter((summary) => summary.earlier?.source === STEAM && summary.earlier.earlier_ms > 0);
  const importedMs = imported.reduce((sum, summary) => sum + (summary.earlier?.earlier_ms ?? 0), 0);
  const importedAt = imported[0]?.earlier?.imported_at;
  const adds = preview?.games.filter((game) => game.earlier_ms > 0) ?? [];
  const addsMs = adds.reduce((sum, game) => sum + game.earlier_ms, 0);

  async function openPreview() {
    setOpen(true);
    setPreview(null);
    setMessage(null);
    try {
      setPreview(await api.previewSteamPlaytime());
    } catch (error) {
      setOpen(false);
      onError(describeError(error));
    }
  }

  async function runImport() {
    setBusy(true);
    try {
      const result = await api.importSteamPlaytime();
      await refresh();
      setOpen(false);
      const added = result.games.filter((game) => game.earlier_ms > 0).length;
      setMessage(`Added earlier playtime to ${numberWords(added)} game${added === 1 ? "" : "s"}.`);
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  async function remove() {
    setBusy(true);
    try {
      await api.removeSteamPlaytime();
      await refresh();
      setMessage("The playtime from Steam is removed. Your tracked sessions stay as they are.");
    } catch (error) {
      onError(describeError(error));
    } finally {
      setBusy(false);
    }
  }

  return (
    <PageSection
      title="Earlier playtime"
      description="Playtime your games had before Vaultime, from Steam's own count. It counts toward each game's total, while the journal and the stats keep to tracked sessions."
    >
      <PageRow
        label="From Steam"
        hint={
          imported.length > 0
            ? `${formatHoursMinutes(importedMs)} for ${numberWords(imported.length)} game${imported.length === 1 ? "" : "s"}${importedAt ? `, read on ${formatCalendarDay(importedAt)}` : ""}.`
            : "Read once from the Steam client on this PC. Time Vaultime tracked already counts only once."
        }
      >
        <span className="flex gap-2">
          {imported.length > 0 && (
            <Button variant="outline" size="sm" onClick={() => void remove()} disabled={busy}>
              <Trash2 className="size-3.5" />
              Remove
            </Button>
          )}
          <Button size="sm" onClick={() => void openPreview()} disabled={busy}>
            <History className="size-3.5" />
            {imported.length > 0 ? "Read again" : "Import from Steam"}
          </Button>
        </span>
      </PageRow>
      {message && <Notice className="mt-4">{message}</Notice>}

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-xl">
          <DialogHeader>
            <DialogTitle>Playtime from Steam</DialogTitle>
            <DialogDescription>
              {preview === null
                ? "Reading Steam's playtime."
                : !preview.found
                  ? "No Steam install with playtime was found on this PC."
                  : preview.games.length === 0
                    ? "Steam counted playtime, but none of it belongs to a game in your library. Discover your Steam games first."
                    : `${preview.account ? `From the Steam account ${preview.account}. ` : ""}Adds ${formatHoursMinutes(addsMs)} to ${numberWords(adds.length)} game${adds.length === 1 ? "" : "s"}.`}
            </DialogDescription>
          </DialogHeader>

          {preview === null ? (
            <p className="flex items-center gap-2 py-3 text-sm text-faint">
              <Loader2 className="size-4 animate-spin" />
              Reading
            </p>
          ) : (
            preview.games.length > 0 && (
              <ol className="max-h-[50vh] overflow-y-auto">
                {preview.games.map((game) => (
                  <li
                    key={game.game_id}
                    className="flex items-baseline justify-between gap-4 border-b border-rule py-2.5 last:border-b-0"
                  >
                    <div className="min-w-0">
                      <div className="truncate text-[15px]">{game.title}</div>
                      <div className="text-xs text-faint">
                        Steam counted {formatHoursMinutes(game.launcher_minutes * MINUTE_MS)}
                        {game.tracked_before_ms > 0 && `, ${formatHoursMinutes(game.tracked_before_ms)} of it tracked here`}
                      </div>
                    </div>
                    <span className="shrink-0 font-mono text-sm tabular-nums">
                      {game.earlier_ms > 0 ? `+${formatHoursMinutes(game.earlier_ms)}` : "nothing new"}
                    </span>
                  </li>
                ))}
              </ol>
            )
          )}

          <DialogFooter>
            <Button variant="ghost" onClick={() => setOpen(false)}>
              Cancel
            </Button>
            <Button onClick={() => void runImport()} disabled={busy || adds.length === 0}>
              {busy && <Loader2 className="size-4 animate-spin" />}
              Add to {numberWords(adds.length)} game{adds.length === 1 ? "" : "s"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </PageSection>
  );
}
