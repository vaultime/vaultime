// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { Phrase } from "@/lib/sentences";

/** A sentence from lib/sentences with its italic part. */
export function PhraseText({ phrase }: { phrase: Phrase }) {
  return (
    <>
      {phrase.before}
      {phrase.em && <em>{phrase.em}</em>}
      {phrase.after}
    </>
  );
}
