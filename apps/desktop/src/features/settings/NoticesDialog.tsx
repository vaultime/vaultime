// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

/** Written by `scripts/third-party-notices.mjs` before every Tauri build. */
const NOTICES_PATH = "/third-party-notices.txt";

/** The notices of the fonts and libraries Vaultime bundles, as its build wrote them. */
export function NoticesDialog({ onClose }: { onClose: () => void }) {
  const [text, setText] = useState<string | null>(null);
  const [missing, setMissing] = useState(false);

  useEffect(() => {
    let cancelled = false;
    fetch(NOTICES_PATH)
      .then((response) => (response.ok ? response.text() : Promise.reject(new Error(String(response.status)))))
      .then((notices) => {
        if (!cancelled) setText(notices);
      })
      .catch(() => {
        if (!cancelled) setMissing(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-3xl">
        <DialogHeader>
          <DialogTitle>Third-party notices</DialogTitle>
          <DialogDescription>
            Vaultime bundles these fonts and libraries, each under its own license.
          </DialogDescription>
        </DialogHeader>
        {missing ? (
          <p className="text-[13px] text-soft">This build carries no notices. The installers do.</p>
        ) : (
          <pre className="max-h-[60vh] overflow-auto font-mono text-[11px] leading-relaxed whitespace-pre-wrap text-soft">
            {text ?? "Reading…"}
          </pre>
        )}
      </DialogContent>
    </Dialog>
  );
}
