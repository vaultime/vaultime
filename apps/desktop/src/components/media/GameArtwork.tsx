// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Gamepad2 } from "lucide-react";
import { cn } from "@/lib/utils";

interface GameArtworkProps {
  src?: string | null;
  alt: string;
  className?: string;
  imageClassName?: string;
  iconClassName?: string;
}

export function GameArtwork({
  src,
  alt,
  className,
  imageClassName,
  iconClassName,
}: GameArtworkProps) {
  return (
    <div
      className={cn(
        "relative overflow-hidden bg-[radial-gradient(circle_at_top,rgba(160,109,255,0.35),transparent_42%),linear-gradient(180deg,rgba(123,78,255,0.22),rgba(18,11,32,0.92))]",
        className,
      )}
    >
      <div className="absolute inset-0 bg-[linear-gradient(180deg,rgba(255,255,255,0.06),transparent_34%,rgba(11,6,20,0.78))]" />
      {src ? (
        <img
          src={src}
          alt={alt}
          className={cn(
            "relative h-full w-full object-cover object-center",
            imageClassName,
          )}
        />
      ) : (
        <div className="relative flex h-full w-full items-center justify-center">
          <Gamepad2
            className={cn("h-14 w-14 text-white/45", iconClassName)}
          />
        </div>
      )}
    </div>
  );
}
