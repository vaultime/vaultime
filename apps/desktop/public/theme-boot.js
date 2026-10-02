// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// Puts the look of the last run on the page before the first paint, so a
// light or recolored Vaultime does not flash the default dark violet first.
// The AppearanceProvider writes the copy. The key is APPEARANCE_CACHE_KEY in
// src/lib/constants.ts.
try {
  const cached = JSON.parse(localStorage.getItem("vaultime.appearance") || "null");
  if (cached && cached.tokens && (cached.mode === "dark" || cached.mode === "light")) {
    const root = document.documentElement;
    for (const [name, value] of Object.entries(cached.tokens)) {
      if (name.startsWith("--")) root.style.setProperty(name, String(value));
    }
    root.style.colorScheme = cached.mode;
    root.classList.toggle("dark", cached.mode === "dark");
  }
} catch {
  // Without the copy the page starts in the default look.
}
