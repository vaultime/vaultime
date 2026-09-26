// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

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
        className={cn("shrink-0 rounded-md border border-white/5 object-cover", className)}
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
        "font-display flex shrink-0 items-end overflow-hidden rounded-md border p-3 leading-none",
        className,
      )}
      style={{ background: tint.fill, borderColor: tint.edge, color: tint.ink }}
    >
      <span className="line-clamp-3 text-balance">{title}</span>
    </span>
  );
}
