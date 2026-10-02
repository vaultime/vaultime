// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { KeyboardEvent } from "react";

const STEPS: Record<string, number> = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 };

/** The radio a key moves to from `current` among `count`, null for other keys. */
export function nextRadio(key: string, current: number, count: number): number | null {
  if (count === 0) return null;
  if (key === "Home") return 0;
  if (key === "End") return count - 1;
  if (!(key in STEPS)) return null;
  return (current + STEPS[key] + count) % count;
}

/**
 * Arrow keys, Home and End for a group of `role="radio"` buttons, the way
 * native radios move. Moving to a radio also picks it.
 */
export function onRadioKeys(event: KeyboardEvent<HTMLElement>) {
  const radios = [...event.currentTarget.querySelectorAll<HTMLElement>('[role="radio"]')];
  const current = radios.findIndex((radio) => radio === document.activeElement);
  if (current < 0) return;
  const next = nextRadio(event.key, current, radios.length);
  if (next === null) return;
  event.preventDefault();
  radios[next].focus();
  radios[next].click();
}

/** The one tab stop of a radio group: the checked radio, or the first when none is. */
export function radioTabIndex(checked: boolean, index: number, anyChecked: boolean): 0 | -1 {
  return checked || (!anyChecked && index === 0) ? 0 : -1;
}
