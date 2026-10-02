// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Puts the look of the last run on the page before the first paint, so a
// light or recolored Vaultime does not flash the default dark violet first.
// The copy holds the tokens of both modes, and System mode takes the one the
// system asks for now. The AppearanceProvider writes the copy. The key is
// APPEARANCE_CACHE_KEY in src/lib/constants.ts.
try {
  const cached = JSON.parse(localStorage.getItem("vaultime.appearance") || "null");
  const choice = cached && cached.appearance && cached.appearance.mode;
  const mode =
    choice === "system" ? (matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light") : choice;
  const tokens = (mode === "dark" || mode === "light") && cached.tokens && cached.tokens[mode];
  if (tokens && typeof tokens === "object") {
    const root = document.documentElement;
    for (const [name, value] of Object.entries(tokens)) {
      if (name.startsWith("--")) root.style.setProperty(name, String(value));
    }
    root.style.colorScheme = mode;
    root.classList.toggle("dark", mode === "dark");
  }
} catch {
  // Without the copy the page starts in the default look.
}
