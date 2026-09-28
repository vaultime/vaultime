// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

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

/** Turns violet while signed in to cloud backup, with the pivot in ink. */
export function Logo({ className, signedIn = false }: { className?: string; signedIn?: boolean }) {
  return (
    <span
      className={cn(
        "inline-flex items-center gap-2.5 transition-colors duration-500",
        signedIn ? "text-violet" : "text-text",
        className,
      )}
      title={signedIn ? "Signed in to cloud backup" : undefined}
    >
      <LogoMark className="size-[28px] shrink-0" pivotClassName={signedIn ? "fill-text" : "fill-violet"} />
      <span className="font-display text-[25px] leading-none font-semibold tracking-[-0.02em]">
        Vaultime
      </span>
    </span>
  );
}
