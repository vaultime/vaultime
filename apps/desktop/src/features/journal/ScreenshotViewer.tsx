// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { StepButton } from "@/components/layout/Page";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import * as api from "@/lib/tauri";
import { formatClockTime } from "@/lib/time";
import type { Screenshot } from "@/lib/types";
import { logFailure } from "@/lib/utils";

/** A session's screenshots one at a time and large, Steam's small copy until the full one is read. */
export function ScreenshotViewer({
  title,
  shots,
  index,
  onIndex,
  onClose,
}: {
  title: string;
  shots: Screenshot[];
  /** The screenshot shown, null while the viewer is closed. */
  index: number | null;
  onIndex: (index: number) => void;
  onClose: () => void;
}) {
  const shot = index === null ? undefined : shots[index];
  const [full, setFull] = useState<{ file: string; url: string } | null>(null);
  useEffect(() => {
    if (!shot) return;
    let cancelled = false;
    api
      .getSteamScreenshot(shot.app_id, shot.file)
      .then((url) => {
        if (!cancelled) setFull({ file: shot.file, url });
      })
      .catch(logFailure("opening a Steam screenshot"));
    return () => {
      cancelled = true;
    };
  }, [shot]);

  const at = index ?? 0;
  const step = (by: number) => {
    const next = at + by;
    if (next >= 0 && next < shots.length) onIndex(next);
  };
  const src = full && shot && full.file === shot.file ? full.url : shot?.thumbnail;

  return (
    <Dialog open={shot !== undefined} onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        className="gap-3 p-3 sm:max-w-[min(1280px,92vw)]"
        onKeyDown={(event) => {
          if (event.key === "ArrowLeft") step(-1);
          if (event.key === "ArrowRight") step(1);
        }}
      >
        <div className="flex min-h-9 items-center gap-3 pr-10 pl-2">
          <DialogTitle className="min-w-0 truncate text-[15px] font-medium">{title}</DialogTitle>
          {shot && <span className="font-mono text-xs text-faint">{formatClockTime(new Date(shot.taken_at_ms))}</span>}
          {shots.length > 1 && (
            <>
              <span className="ml-auto text-xs text-faint tabular-nums">
                {at + 1} of {shots.length}
              </span>
              <StepButton label="Previous screenshot" disabled={at === 0} onClick={() => step(-1)}>
                <ChevronLeft className="size-[18px]" strokeWidth={1.8} />
              </StepButton>
              <StepButton label="Next screenshot" disabled={at >= shots.length - 1} onClick={() => step(1)}>
                <ChevronRight className="size-[18px]" strokeWidth={1.8} />
              </StepButton>
            </>
          )}
        </div>
        {src && (
          <img
            src={src}
            alt={`Screenshot of ${title}`}
            className="max-h-[78vh] w-full rounded-lg bg-scrim object-contain"
          />
        )}
      </DialogContent>
    </Dialog>
  );
}
