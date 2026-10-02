// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import { Minus, Plus } from "lucide-react";
import { StepButton } from "@/components/layout/Page";
import { IDLE_STEP_MIN_MINUTES, MIN_IDLE_THRESHOLD_SECS, SECONDS_PER_MINUTE } from "@/lib/constants";

/** "5", "1.5": the idle time in minutes as the field shows it. */
function minutesText(seconds: number): string {
  return String(Number((seconds / SECONDS_PER_MINUTE).toFixed(2)));
}

/**
 * The idle time in minutes, stepped a minute at a time or typed. Reports each
 * change in seconds, a typed one when the field loses focus or on Enter.
 */
export function IdleStepper({
  id,
  seconds,
  onChange,
}: {
  /** Id of the field, so the row label focuses it. */
  id: string;
  seconds: number;
  onChange: (seconds: number) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const minutes = seconds / SECONDS_PER_MINUTE;
  const text = draft ?? minutesText(seconds);

  function step(by: 1 | -1) {
    const next = by > 0 ? Math.floor(minutes) + 1 : Math.ceil(minutes) - 1;
    onChange(Math.max(IDLE_STEP_MIN_MINUTES, next) * SECONDS_PER_MINUTE);
  }

  function commit() {
    if (draft === null) return;
    setDraft(null);
    const typed = Math.round(Number(draft.trim().replace(",", ".")) * SECONDS_PER_MINUTE);
    // Empty or unreadable input goes back to the saved value.
    if (!draft.trim() || !Number.isFinite(typed) || typed <= 0) return;
    const next = Math.max(MIN_IDLE_THRESHOLD_SECS, typed);
    if (next !== seconds) onChange(next);
  }

  return (
    <span className="inline-flex items-center gap-3">
      <StepButton
        label="One minute less"
        disabled={minutes <= IDLE_STEP_MIN_MINUTES}
        onClick={() => step(-1)}
        className="size-8"
      >
        <Minus className="size-3.5" strokeWidth={1.8} />
      </StepButton>
      <span className="inline-flex items-baseline gap-1.5">
        <input
          id={id}
          inputMode="decimal"
          value={text}
          onChange={(event) => setDraft(event.target.value)}
          onBlur={commit}
          onKeyDown={(event) => {
            if (event.key === "Enter") commit();
            if (event.key === "Escape") setDraft(null);
            if (event.key === "ArrowUp" || event.key === "ArrowDown") {
              event.preventDefault();
              setDraft(null);
              step(event.key === "ArrowUp" ? 1 : -1);
            }
          }}
          style={{ width: `${Math.max(text.length, 2)}ch` }}
          className="border-b border-hairline bg-transparent text-center font-mono text-[17px] text-text tabular-nums transition-colors outline-none hover:border-hairline-strong focus:border-violet"
        />
        <span className="text-sm text-faint">min</span>
      </span>
      <StepButton label="One minute more" disabled={false} onClick={() => step(1)} className="size-8">
        <Plus className="size-3.5" strokeWidth={1.8} />
      </StepButton>
    </span>
  );
}
