// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { Session } from "@/lib/types";

/** False for a finished session whose time was all taken out. It stays in the history as no play. */
export function countsAsPlay(session: Session): boolean {
  return !session.ended_at_wall || session.runtime_ms > 0;
}
