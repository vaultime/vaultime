// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { StillBlur } from "@/components/media/StillBlur";
import { useAppearance } from "@/features/appearance/appearance-context";
import { BACKGROUND_BLUR_BLEED, BACKGROUND_SATURATION } from "@/lib/constants";
import { stillBlur } from "@/lib/still-blur";

/** The art of the game in front or the picture behind the whole app, blurred and dimmed by the window tone. */
export function Backdrop() {
  const { backdrop, backdropDim, appearance } = useAppearance();
  if (!backdrop) return null;
  // A blur fades out at the edges, so the picture reaches past them.
  const bleed = appearance.blur * BACKGROUND_BLUR_BLEED;
  if (stillBlur()) {
    return (
      <div aria-hidden="true" className="pointer-events-none absolute inset-0 -z-10 overflow-hidden">
        <StillBlur
          src={backdrop}
          blur={appearance.blur}
          saturation={BACKGROUND_SATURATION}
          className="absolute inset-0 size-full"
        />
        <div className="absolute inset-0 bg-window" style={{ opacity: backdropDim / 100 }} />
      </div>
    );
  }
  return (
    <div aria-hidden="true" className="pointer-events-none absolute inset-0 -z-10 overflow-hidden">
      <img
        src={backdrop}
        alt=""
        className="absolute max-w-none object-cover"
        style={{
          inset: -bleed,
          width: `calc(100% + ${bleed * 2}px)`,
          height: `calc(100% + ${bleed * 2}px)`,
          filter: `${appearance.blur > 0 ? `blur(${appearance.blur}px) ` : ""}saturate(${BACKGROUND_SATURATION})`,
          // A layer of its own, so the blur is not worked out again whenever
          // the page in front of it repaints.
          willChange: "transform",
        }}
      />
      <div className="absolute inset-0 bg-window" style={{ opacity: backdropDim / 100 }} />
    </div>
  );
}
