// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { ImageIcon } from "lucide-react";
import { formatClockTime } from "@/lib/time";
import type { SessionScreenshots } from "@/lib/types";
import { numberWords } from "@/lib/words";

/** The Steam screenshots of a session in small, oldest first. */
export function ScreenshotStrip({ screenshots }: { screenshots: SessionScreenshots }) {
  const more = screenshots.total - screenshots.shots.length;
  return (
    <div className="mt-2.5 flex flex-wrap items-center gap-2">
      <ul aria-label="Screenshots from this session" className="flex flex-wrap gap-2">
        {screenshots.shots.map((shot) => (
          <li key={shot.file}>
            {shot.thumbnail ? (
              <img
                src={shot.thumbnail}
                alt={`Screenshot from ${formatClockTime(new Date(shot.taken_at_ms))}`}
                className="h-[54px] w-24 rounded-md border border-glint/5 object-cover"
              />
            ) : (
              <span
                role="img"
                aria-label={`Screenshot from ${formatClockTime(new Date(shot.taken_at_ms))}`}
                className="flex h-[54px] w-24 items-center justify-center rounded-md border border-hairline bg-raised text-faint"
              >
                <ImageIcon className="size-4" aria-hidden="true" />
              </span>
            )}
          </li>
        ))}
      </ul>
      {more > 0 && <span className="text-xs text-faint">and {numberWords(more)} more</span>}
    </div>
  );
}
