// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Badge } from "@/components/ui/badge";
import { getIntegrityMeta, normalizeIntegrityStatus } from "@/lib/integrity";

const VARIANTS: Record<string, "secondary" | "amber" | "sky" | "outline"> = {
  local: "secondary",
  suspicious: "amber",
  recovered: "sky",
  edited: "outline",
  manual: "outline",
};

/**
 * The trust label of a session: Local, Suspicious, Recovered, Edited or
 * Manual, and Checking while its chain is not checked yet. A check only ever
 * finds a session Suspicious, so a Suspicious one says so right away.
 */
export function IntegrityBadge({
  status,
  checking = false,
  className,
}: {
  status: string;
  checking?: boolean;
  className?: string;
}) {
  const normalized = normalizeIntegrityStatus(status);
  if (checking && normalized !== "suspicious") {
    return (
      <Badge variant="outline" title="Vaultime checks the record of every session after it starts." className={className}>
        Checking
      </Badge>
    );
  }
  const meta = getIntegrityMeta(normalized);
  return (
    <Badge variant={VARIANTS[normalized]} title={meta.description} className={className}>
      {meta.label}
    </Badge>
  );
}
