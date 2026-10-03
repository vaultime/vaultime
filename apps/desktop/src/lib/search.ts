// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

/** Lowercase and without accents, so "pokemon" finds "Pokémon". */
function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

/** True when every word of `query` is somewhere in `text`. An empty query matches everything. */
export function matchesSearch(text: string, query: string): boolean {
  const haystack = fold(text);
  return fold(query)
    .split(/\s+/)
    .filter(Boolean)
    .every((word) => haystack.includes(word));
}
