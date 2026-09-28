// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { cn } from "@/lib/utils";

/** Clock hands at five past eleven form a V inside a rounded vault frame. The
 * view box ends at the outer edge of the frame, so the mark has no margin. */
export function LogoMark({ className }: { className?: string }) {
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
      <circle cx="32" cy="32.5" r="3.8" className="fill-violet" />
    </svg>
  );
}

export function Logo({ className }: { className?: string }) {
  return (
    <span className={cn("inline-flex items-center gap-2.5 text-text", className)}>
      <LogoMark className="size-[28px] shrink-0" />
      <span className="font-display text-[25px] leading-none font-semibold tracking-[-0.02em]">
        Vaultime
      </span>
    </span>
  );
}
