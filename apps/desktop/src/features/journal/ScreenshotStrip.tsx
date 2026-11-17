// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { ImageIcon } from "lucide-react";
import { formatClockTime } from "@/lib/time";
import type { SessionScreenshots } from "@/lib/types";
import { numberWords } from "@/lib/words";

/** The Steam screenshots of a session in small, oldest first, each opening the viewer. */
export function ScreenshotStrip({
  screenshots,
  onOpen,
}: {
  screenshots: SessionScreenshots;
  onOpen: (index: number) => void;
}) {
  const more = screenshots.total - screenshots.shots.length;
  return (
    <div className="mt-2.5 flex flex-wrap items-center gap-2">
      <ul aria-label="Screenshots from this session" className="flex flex-wrap gap-2">
        {screenshots.shots.map((shot, index) => (
          <li key={shot.file}>
            <button
              type="button"
              aria-label={`Screenshot from ${formatClockTime(new Date(shot.taken_at_ms))}`}
              onClick={() => onOpen(index)}
              className="block cursor-pointer overflow-hidden rounded-md border border-glint/5 transition-[border-color] hover:border-hairline-strong focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
            >
              {shot.thumbnail ? (
                <img src={shot.thumbnail} alt="" className="h-[54px] w-24 object-cover" />
              ) : (
                <span className="flex h-[54px] w-24 items-center justify-center bg-raised text-faint">
                  <ImageIcon className="size-4" aria-hidden="true" />
                </span>
              )}
            </button>
          </li>
        ))}
      </ul>
      {more > 0 && <span className="text-xs text-faint">and {numberWords(more)} more</span>}
    </div>
  );
}
