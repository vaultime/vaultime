// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useRef, useState } from "react";
import { Minus, Plus } from "lucide-react";
import { StepButton } from "@/components/layout/Page";
import {
  IDLE_STEP_MIN_MINUTES,
  MAX_IDLE_THRESHOLD_SECS,
  MIN_IDLE_THRESHOLD_SECS,
  SECONDS_PER_MINUTE,
} from "@/lib/constants";

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
  const field = useRef<HTMLInputElement>(null);
  const minutes = seconds / SECONDS_PER_MINUTE;
  const text = draft ?? minutesText(seconds);
  const atMin = minutes <= IDLE_STEP_MIN_MINUTES;
  const atMax = seconds >= MAX_IDLE_THRESHOLD_SECS;

  function step(by: 1 | -1, fromButton = false) {
    const next = by > 0 ? Math.floor(minutes) + 1 : Math.ceil(minutes) - 1;
    const nextSeconds = Math.min(MAX_IDLE_THRESHOLD_SECS, Math.max(IDLE_STEP_MIN_MINUTES, next) * SECONDS_PER_MINUTE);
    onChange(nextSeconds);
    // A button that reaches its limit turns off, so keyboard focus moves to the field instead of the page.
    const limit = by > 0 ? nextSeconds >= MAX_IDLE_THRESHOLD_SECS : nextSeconds <= IDLE_STEP_MIN_MINUTES * SECONDS_PER_MINUTE;
    if (fromButton && limit) field.current?.focus();
  }

  function commit() {
    if (draft === null) return;
    setDraft(null);
    const typed = Number(draft.trim().replace(",", "."));
    // Empty or unreadable input goes back to the saved value, anything else is kept in range.
    if (!draft.trim() || !Number.isFinite(typed)) return;
    const next = Math.min(
      MAX_IDLE_THRESHOLD_SECS,
      Math.max(MIN_IDLE_THRESHOLD_SECS, Math.round(typed * SECONDS_PER_MINUTE)),
    );
    if (next !== seconds) onChange(next);
  }

  return (
    <span className="inline-flex items-center gap-3">
      <StepButton label="One minute less" disabled={atMin} onClick={() => step(-1, true)} className="size-8">
        <Minus className="size-3.5" strokeWidth={1.8} />
      </StepButton>
      <span className="inline-flex items-baseline gap-1.5">
        <input
          ref={field}
          id={id}
          role="spinbutton"
          inputMode="decimal"
          aria-valuenow={minutes}
          aria-valuemin={MIN_IDLE_THRESHOLD_SECS / SECONDS_PER_MINUTE}
          aria-valuemax={MAX_IDLE_THRESHOLD_SECS / SECONDS_PER_MINUTE}
          aria-valuetext={`${text} minute${text === "1" ? "" : "s"}`}
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
        <span aria-hidden="true" className="text-sm text-faint">
          min
        </span>
      </span>
      <StepButton label="One minute more" disabled={atMax} onClick={() => step(1, true)} className="size-8">
        <Plus className="size-3.5" strokeWidth={1.8} />
      </StepButton>
    </span>
  );
}
