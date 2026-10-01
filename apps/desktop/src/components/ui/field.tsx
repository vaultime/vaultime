// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ReactNode } from "react";
import { FieldContext } from "@/components/ui/field-context";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";

/** A labeled control. A problem replaces the hint below it until it is fixed. */
export function Field({
  id,
  label,
  hint,
  problem,
  children,
}: {
  id: string;
  label: string;
  hint?: ReactNode;
  problem?: string | null;
  children: ReactNode;
}) {
  const message = problem || hint;
  const messageId = message ? `${id}-message` : undefined;
  return (
    <FieldContext value={{ messageId, invalid: Boolean(problem) }}>
      <div className="grid gap-1.5">
        <Label htmlFor={id}>{label}</Label>
        {children}
        {message && (
          <p id={messageId} className={cn("text-xs", problem ? "text-amber" : "text-faint")}>
            {message}
          </p>
        )}
      </div>
    </FieldContext>
  );
}
