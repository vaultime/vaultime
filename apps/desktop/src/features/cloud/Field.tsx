// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import type { ReactNode } from "react";
import { Label } from "@/components/ui/label";

export function Field({ id, label, children }: { id: string; label: string; children: ReactNode }) {
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={id}>{label}</Label>
      {children}
    </div>
  );
}
