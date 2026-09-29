// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Bookmark, CircleOff, Flag, Gamepad2, type LucideProps } from "lucide-react";
import type { GameStatus } from "@/lib/types";

const ICONS = { backlog: Bookmark, playing: Gamepad2, finished: Flag, dropped: CircleOff };

export function GameStatusIcon({ status, ...props }: { status: GameStatus } & LucideProps) {
  const Icon = ICONS[status];
  return <Icon aria-hidden="true" {...props} />;
}
