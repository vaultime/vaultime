// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import { Cover } from "@/components/media/Cover";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { useLibrary } from "@/features/library/library-context";
import { VAULTIME_URL, WHATS_NEW_COVERS } from "@/lib/constants";
import { formatLongDate } from "@/lib/time";
import { FIXES_HEADING, type Release } from "@/lib/whats-new";

/** The changes of a release, under the covers of the games played last. */
export function WhatsNewDialog({ release, onClose }: { release: Release; onClose: () => void }) {
  const { visible } = useLibrary();
  const covers = visible
    .filter((summary) => summary.cover && summary.lastPlayedAt)
    .sort((a, b) => (b.lastPlayedAt ?? "").localeCompare(a.lastPlayedAt ?? ""))
    .slice(0, WHATS_NEW_COVERS);
  const fixes = release.groups.find((group) => group.heading === FIXES_HEADING)?.notes.length ?? 0;
  const groups = release.groups.filter((group) => group.heading !== FIXES_HEADING);

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
    >
      <DialogContent className="sm:max-w-xl">
        {covers.length > 0 && (
          <div
            aria-hidden="true"
            className="-mx-6 -mt-6 flex h-32 justify-center gap-2 overflow-hidden px-6 pt-6 [mask-image:linear-gradient(to_bottom,black_35%,transparent)]"
          >
            {covers.map((summary) => (
              <Cover
                key={summary.game.id}
                title={summary.game.title}
                src={summary.cover}
                className="aspect-[3/4] w-[84px] shrink-0 p-2 text-[11px]"
              />
            ))}
          </div>
        )}
        <DialogHeader>
          <DialogTitle>What's new in Vaultime {release.version}</DialogTitle>
          <DialogDescription>
            {release.date ? `Out since ${formatLongDate(release.date)}.` : "The changes since the last release."}
          </DialogDescription>
        </DialogHeader>

        <div className="-mx-6 max-h-[min(24rem,calc(100dvh-20rem))] overflow-y-auto border-y border-rule px-6">
          {groups.map((group) => (
            <section key={group.heading} aria-label={group.heading} className="border-b border-rule py-4 last:border-b-0">
              <h3 className="label-small">{group.heading}</h3>
              <ul className="mt-2 flex flex-col gap-3">
                {group.notes.map((note) => (
                  <li key={`${note.title}${note.text}`}>
                    {note.title && <p className="text-sm font-medium text-text">{note.title}</p>}
                    <p className="text-[13px] leading-relaxed text-soft">{note.text}</p>
                  </li>
                ))}
              </ul>
            </section>
          ))}
          {fixes > 0 && (
            <p className="border-t border-rule py-4 text-[13px] text-soft">
              And {fixes === 1 ? "one fix" : `${fixes} fixes`}, listed in the full changelog.
            </p>
          )}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={() => void openUrl(`${VAULTIME_URL}/changelog.html`)}>
            <ExternalLink className="size-3.5" />
            Full changelog
          </Button>
          <DialogClose render={<Button />}>Close</DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
