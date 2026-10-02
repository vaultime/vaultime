// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The colored header that opens the library and every game page.

import { useLayoutEffect, useRef, type ReactNode } from "react";
import { Link } from "react-router";
import { HERO_TITLE_MIN_FONT_PX } from "@/lib/constants";
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
  const seeThrough = (color: string) => `color-mix(in oklab, ${color} var(--header-alpha), transparent)`;
  return (
    <section
      className={cn("relative isolate overflow-hidden border-b px-8 pt-12 pb-10 xl:px-14", className)}
      style={{
        borderColor: tint.fill,
        background: `radial-gradient(120% 150% at 90% 0%, ${seeThrough(tint.fill)} 0%, ${seeThrough(tint.wash)} 60%)`,
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

/**
 * Every title gets the same size and one line, so the header keeps its height
 * from game to game. A title too long for the line shrinks until it fits, and
 * one that does not fit even then wraps at the smallest size.
 */
export function TintedTitle({ tint, text }: { tint: GameTint; text: string }) {
  const lineRef = useRef<HTMLHeadingElement>(null);
  const textRef = useRef<HTMLSpanElement>(null);

  useLayoutEffect(() => {
    const line = lineRef.current;
    const title = textRef.current;
    if (!line || !title) return;

    function fit() {
      if (!line || !title) return;
      title.style.fontSize = "";
      title.style.removeProperty("text-wrap");
      const available = line.clientWidth;
      let needed = title.offsetWidth;
      let size = parseFloat(getComputedStyle(line).fontSize);
      // Letter spacing and hinting do not scale exactly, so one step can fall short.
      while (needed > available && size > HERO_TITLE_MIN_FONT_PX) {
        size = Math.max(Math.floor((size * available) / needed), HERO_TITLE_MIN_FONT_PX);
        title.style.fontSize = `${size}px`;
        needed = title.offsetWidth;
      }
      if (needed > available) {
        title.style.setProperty("text-wrap", "balance");
      }
    }

    fit();
    const observer = new ResizeObserver(fit);
    observer.observe(line);
    // The font may still be loading, and it sets different widths.
    void document.fonts.ready.then(fit);
    return () => observer.disconnect();
  }, [text]);

  return (
    <h1
      ref={lineRef}
      className="font-display mt-3.5 text-[clamp(48px,5.4vw,80px)] leading-[0.95] whitespace-nowrap"
      style={{ color: tint.ink }}
    >
      {/* The line keeps the full size height, the shrunk title sits on its baseline. */}
      <span ref={textRef} className="inline-block">
        {text}
      </span>
    </h1>
  );
}

export function TintedSentence({
  tint,
  className,
  children,
}: {
  tint: GameTint;
  className?: string;
  children: ReactNode;
}) {
  return (
    <p
      className={cn("font-prose mt-5 max-w-[620px] text-2xl leading-[1.3] text-pretty", className)}
      style={{ color: tint.soft }}
    >
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
  const className = cn(BUTTON, solid ? "font-semibold hover:opacity-90" : "border font-medium hover:bg-glint/5");
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
