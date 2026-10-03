// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

const FIND_EVENT = "vaultime-find";

/** Ctrl+F asks the page for its own search first. False when no page has one. */
export function requestFind(): boolean {
  return !window.dispatchEvent(new Event(FIND_EVENT, { cancelable: true }));
}

/** Answers Ctrl+F with the page's own search until the returned function is called. */
export function onFind(handler: () => void): () => void {
  const listener = (event: Event) => {
    event.preventDefault();
    handler();
  };
  window.addEventListener(FIND_EVENT, listener);
  return () => window.removeEventListener(FIND_EVENT, listener);
}
