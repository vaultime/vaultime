// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Badge } from "@/components/ui/badge";
import { getIntegrityMeta, normalizeIntegrityStatus } from "@/lib/integrity";

const VARIANTS: Record<string, "secondary" | "amber" | "sky"> = {
  local: "secondary",
  suspicious: "amber",
  recovered: "sky",
};

/** The trust label of a session: Local, Suspicious or Recovered. */
export function IntegrityBadge({ status, className }: { status: string; className?: string }) {
  const normalized = normalizeIntegrityStatus(status);
  const meta = getIntegrityMeta(normalized);
  return (
    <Badge variant={VARIANTS[normalized]} title={meta.description} className={className}>
      {meta.label}
    </Badge>
  );
}
