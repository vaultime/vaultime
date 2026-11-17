// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { cn } from "@/lib/utils";

/** Clock hands at five past eleven form a V inside a rounded vault frame. The
 * view box ends at the outer edge of the frame, so the mark has no margin. */
export function LogoMark({
  className,
  pivotClassName = "fill-violet",
}: {
  className?: string;
  pivotClassName?: string;
}) {
  return (
    <svg viewBox="4.75 4.75 54.5 54.5" fill="none" aria-hidden="true" className={className}>
      <rect x="7" y="7" width="50" height="50" rx="15" stroke="currentColor" strokeWidth="4.5" />
      <path
        d="M25.5 21.3L32 32.5L41 17"
        stroke="currentColor"
        strokeWidth="6.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <path d="M32 44.5v3.5" stroke="currentColor" strokeWidth="4.5" strokeLinecap="round" />
      <circle cx="32" cy="32.5" r="3.8" className={pivotClassName} />
    </svg>
  );
}

/** In the accent, with the pivot in the text color, as the icons of the window show it. */
export function Logo({ className }: { className?: string }) {
  return (
    <span className={cn("inline-flex items-center gap-2 text-violet transition-colors duration-500", className)}>
      <LogoMark className="size-[22px] shrink-0" pivotClassName="fill-text" />
      <span className="font-wordmark text-[19.5px] leading-none">Vaultime</span>
    </span>
  );
}
