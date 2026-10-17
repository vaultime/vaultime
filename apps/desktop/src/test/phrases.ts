// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { Phrase } from "@/lib/sentences";

/** A phrase as plain text. */
export function phraseString(phrase: Phrase): string {
  return `${phrase.before}${phrase.em ?? ""}${phrase.after ?? ""}`;
}
