// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

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
