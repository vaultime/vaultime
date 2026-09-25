// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Badge } from "@/components/ui/badge";
import { getIntegrityMeta } from "@/lib/integrity";
import { cn } from "@/lib/utils";

interface IntegrityBadgeProps {
  status: string;
  className?: string;
}

export function IntegrityBadge({
  status,
  className,
}: IntegrityBadgeProps) {
  const meta = getIntegrityMeta(status);

  return (
    <Badge
      variant="outline"
      title={meta.description}
      className={cn(
        "border px-2.5 py-0.5 text-[11px] font-medium tracking-[0.16em] uppercase",
        meta.className,
        className,
      )}
    >
      {meta.label}
    </Badge>
  );
}
