// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import type { ReactNode } from "react";
import { Switch } from "@/components/ui/switch";
import { setUpAgain, TAKEOVER_DECLINED } from "@/lib/devices";
import type { PcTakeover } from "@/lib/types";

/** Asks whether a fresh install restoring a backup takes over the backup's PC. */
export function TakeoverChoice({
  id,
  takeover,
  checked,
  disabled,
  onCheckedChange,
  children,
}: {
  id: string;
  /** The PC the restore would take over, null when only the backup's PC is known to differ. */
  takeover: PcTakeover | null;
  checked: boolean;
  disabled: boolean;
  onCheckedChange: (checked: boolean) => void;
  /** What taking over means, shown while it is checked. */
  children: ReactNode;
}) {
  return (
    <div className="grid max-w-[560px] gap-2">
      <label htmlFor={id} className="flex items-center justify-between gap-4">
        <span className="min-w-0 text-[15px]">{setUpAgain(takeover)}</span>
        <Switch id={id} checked={checked} disabled={disabled} onCheckedChange={onCheckedChange} />
      </label>
      {checked ? children : <p className="text-[13px] leading-relaxed text-faint">{TAKEOVER_DECLINED}</p>}
    </div>
  );
}
