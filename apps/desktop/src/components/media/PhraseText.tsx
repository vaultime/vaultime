// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { CSSProperties } from "react";
import type { Phrase } from "@/lib/sentences";

/**
 * A sentence from lib/sentences with its italic part, which may take a color
 * of its own, or a blend of colors painted into its letters.
 */
export function PhraseText({ phrase, emColor, emFill }: { phrase: Phrase; emColor?: string; emFill?: string }) {
  const style: CSSProperties | undefined = emFill
    ? {
        color: emColor,
        backgroundImage: emFill,
        WebkitBackgroundClip: "text",
        backgroundClip: "text",
        WebkitTextFillColor: "transparent",
      }
    : emColor
      ? { color: emColor }
      : undefined;
  return (
    <>
      {phrase.before}
      {phrase.em && <em style={style}>{phrase.em}</em>}
      {phrase.after}
    </>
  );
}
