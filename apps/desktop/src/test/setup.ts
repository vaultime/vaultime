// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// A German system: English words, day before month and a 24 hour clock.
Object.defineProperty(globalThis, "navigator", {
  value: { language: "de-DE" },
  configurable: true,
});
