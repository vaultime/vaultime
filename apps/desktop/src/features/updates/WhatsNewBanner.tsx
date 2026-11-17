// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { Sparkles, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useLibrary } from "@/features/library/library-context";
import { useSettings } from "@/features/settings/settings-context";
import { SETTING_KEYS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import { loadReleaseNotes, type Release } from "@/lib/whats-new";
import { WhatsNewDialog } from "./WhatsNewDialog";

/**
 * After an update, a line over the page that opens the release's changes.
 * A fresh install has nothing to catch up on and shows none.
 */
export function WhatsNewBanner() {
  const { values, read, save } = useSettings();
  const { loaded, games } = useLibrary();
  const [version, setVersion] = useState<string | null>(null);
  const [release, setRelease] = useState<Release | null>(null);
  const [open, setOpen] = useState(false);
  const seen = values[SETTING_KEYS.whatsNewSeen];
  const fresh = seen === undefined && games.length === 0;

  useEffect(() => {
    api
      .getAppVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (!version || !read || !loaded || seen === version) return;
    if (fresh) {
      save(SETTING_KEYS.whatsNewSeen, version).catch(() => {});
      return;
    }
    let cancelled = false;
    loadReleaseNotes(version)
      .then((notes) => {
        if (cancelled) return;
        if (notes) setRelease(notes);
        else save(SETTING_KEYS.whatsNewSeen, version).catch(() => {});
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [version, read, loaded, seen, fresh, save]);

  if (!release || !version) return null;
  const done = () => {
    setOpen(false);
    save(SETTING_KEYS.whatsNewSeen, version).catch(() => {});
  };

  return (
    <>
      {seen !== version && (
        <div className="flex items-center justify-between gap-4 border-b border-violet/30 bg-violet/10 px-8 py-2.5 xl:px-14">
          <p className="text-sm text-text">
            Vaultime is now <span className="font-semibold">v{version}</span>.
          </p>
          <div className="flex items-center gap-2">
            <Button size="sm" onClick={() => setOpen(true)}>
              <Sparkles className="size-3.5" />
              See what's new
            </Button>
            <button
              type="button"
              onClick={done}
              aria-label="Dismiss"
              className="rounded-full p-1.5 text-faint hover:bg-glint/10 hover:text-text"
            >
              <X className="size-4" />
            </button>
          </div>
        </div>
      )}
      {open && <WhatsNewDialog release={release} onClose={done} />}
    </>
  );
}
