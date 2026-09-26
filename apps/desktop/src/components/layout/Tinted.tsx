// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

// The colored header that opens the library and every game page.

import type { ReactNode } from "react";
import { Link } from "react-router";
import { HERO_TITLE_LARGE_MAX_CHARS, HERO_TITLE_MEDIUM_MAX_CHARS } from "@/lib/constants";
import type { GameTint } from "@/lib/game-tint";
import { cn } from "@/lib/utils";

export function TintedHeader({
  tint,
  backdrop,
  className,
  children,
}: {
  tint: GameTint;
  /** Cover art, blurred far behind the header. */
  backdrop?: string | null;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section
      className={cn("relative isolate overflow-hidden border-b px-8 pt-12 pb-10 xl:px-14", className)}
      style={{
        borderColor: tint.fill,
        background: `radial-gradient(120% 150% at 90% 0%, ${tint.fill} 0%, ${tint.wash} 60%)`,
      }}
    >
      {backdrop && (
        <img
          src={backdrop}
          alt=""
          aria-hidden="true"
          className="absolute inset-0 -z-10 size-full scale-125 object-cover opacity-[0.18] blur-3xl"
        />
      )}
      {children}
    </section>
  );
}

export function TintedOverline({ tint, children }: { tint: GameTint; children: ReactNode }) {
  return (
    <div className="flex items-center gap-2.5 text-xs tracking-[0.16em] uppercase" style={{ color: tint.muted }}>
      {children}
    </div>
  );
}

export function TintedTitle({ tint, text }: { tint: GameTint; text: string }) {
  // Long titles step down so they stay on two lines.
  const size =
    text.length <= HERO_TITLE_LARGE_MAX_CHARS
      ? "text-[clamp(56px,7vw,104px)]"
      : text.length <= HERO_TITLE_MEDIUM_MAX_CHARS
        ? "text-[clamp(48px,5.4vw,80px)]"
        : "text-[clamp(40px,4.2vw,60px)]";
  return (
    <h1
      className={cn("font-display mt-3.5 leading-[0.95] font-medium tracking-[-0.03em] text-balance", size)}
      style={{ color: tint.ink }}
    >
      {text}
    </h1>
  );
}

export function TintedSentence({ tint, children }: { tint: GameTint; children: ReactNode }) {
  return (
    <p className="font-display mt-5 max-w-[620px] text-2xl leading-[1.3] text-pretty" style={{ color: tint.soft }}>
      {children}
    </p>
  );
}

const BUTTON =
  "inline-flex h-11 items-center gap-2 rounded-full px-5 text-sm transition focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-transparent focus-visible:outline-none";

/** A pill in the header's colors. Solid for the main action, outlined otherwise. */
export function TintedButton({
  tint,
  solid = false,
  to,
  onClick,
  children,
}: {
  tint: GameTint;
  solid?: boolean;
  to?: string;
  onClick?: () => void;
  children: ReactNode;
}) {
  const className = cn(BUTTON, solid ? "font-semibold hover:opacity-90" : "border font-medium hover:bg-white/5");
  const style = solid
    ? { background: tint.ink, color: tint.wash }
    : { borderColor: tint.edge, color: tint.ink };

  if (to) {
    return (
      <Link to={to} className={className} style={style}>
        {children}
      </Link>
    );
  }
  return (
    <button type="button" onClick={onClick} className={className} style={style}>
      {children}
    </button>
  );
}
