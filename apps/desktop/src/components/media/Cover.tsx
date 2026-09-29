// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { tintForTitle } from "@/lib/game-tint";
import { cn } from "@/lib/utils";

interface CoverProps {
  title: string;
  src?: string | null;
  /** `tile` is a small square with the first letter, `card` a portrait cover. */
  variant?: "tile" | "card";
  className?: string;
}

/** Game artwork, or a typeset cover in the game's tint when there is none. */
export function Cover({ title, src, variant = "card", className }: CoverProps) {
  const tint = tintForTitle(title);

  if (src) {
    return (
      <img
        src={src}
        alt=""
        // Padding in className is for the typeset placeholder, never for artwork.
        className={cn("shrink-0 rounded-md border border-white/5 object-cover", className, "p-0")}
      />
    );
  }

  if (variant === "tile") {
    return (
      <span
        aria-hidden="true"
        className={cn(
          "font-display flex shrink-0 items-center justify-center rounded-md border",
          className,
        )}
        style={{ background: tint.fill, borderColor: tint.edge, color: tint.ink }}
      >
        {title.charAt(0).toUpperCase()}
      </span>
    );
  }

  return (
    <span
      aria-hidden="true"
      className={cn(
        "font-display flex shrink-0 items-end overflow-hidden rounded-md border p-3",
        className,
        // After className, a font size there would drop it otherwise.
        "leading-none",
      )}
      style={{ background: tint.fill, borderColor: tint.edge, color: tint.ink }}
    >
      <span className="line-clamp-3 text-balance">{title}</span>
    </span>
  );
}
