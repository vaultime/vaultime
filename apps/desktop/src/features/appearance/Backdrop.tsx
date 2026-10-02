// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useAppearance } from "@/features/appearance/appearance-context";

/** The background picture behind the whole app, blurred and dimmed by the ground. */
export function Backdrop() {
  const { background, appearance } = useAppearance();
  if (!background) return null;
  // A blur fades out at the edges, so the picture reaches past them.
  const bleed = appearance.blur * 2;
  return (
    <div aria-hidden="true" className="pointer-events-none absolute inset-0 -z-10 overflow-hidden">
      <img
        src={background}
        alt=""
        className="absolute max-w-none object-cover"
        style={{
          inset: -bleed,
          width: `calc(100% + ${bleed * 2}px)`,
          height: `calc(100% + ${bleed * 2}px)`,
          filter: appearance.blur > 0 ? `blur(${appearance.blur}px)` : undefined,
        }}
      />
      <div className="absolute inset-0 bg-ink" style={{ opacity: appearance.dim / 100 }} />
    </div>
  );
}
