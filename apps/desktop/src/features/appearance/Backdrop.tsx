// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { StillBlur } from "@/components/media/StillBlur";
import { useAppearance } from "@/features/appearance/appearance-context";
import { BACKGROUND_SATURATION } from "@/lib/constants";

/** The art of the game in front or the picture behind the whole app, blurred and dimmed by the window tone. */
export function Backdrop() {
  const { backdrop, backdropDim, appearance } = useAppearance();
  if (!backdrop) return null;
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
