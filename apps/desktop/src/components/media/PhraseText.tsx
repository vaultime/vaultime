// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { Phrase } from "@/lib/sentences";

/** A sentence from lib/sentences with its italic part, which may take a color of its own. */
export function PhraseText({ phrase, emColor }: { phrase: Phrase; emColor?: string }) {
  return (
    <>
      {phrase.before}
      {phrase.em && <em style={emColor ? { color: emColor } : undefined}>{phrase.em}</em>}
      {phrase.after}
    </>
  );
}
