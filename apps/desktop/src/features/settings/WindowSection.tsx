// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { Notice, PageSection } from "@/components/layout/Page";
import * as api from "@/lib/tauri";
import type { WindowSizeChoice, WindowSizeState } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";

const NAMES: Record<WindowSizeChoice, string> = {
  compact: "Compact",
  standard: "Standard",
  large: "Large",
  extra_large: "Extra large",
  free: "Free",
};

const HINTS: Partial<Record<WindowSizeChoice, string>> = {
  standard: "The size Vaultime is laid out for.",
  free: "Drag the edges or maximize it, any size goes.",
};

/** A fixed window size like a game client, or a window the player sizes. */
export function WindowSection() {
  const [state, setState] = useState<WindowSizeState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    api.getWindowSize().then(
      (next) => {
        if (!cancelled) setState(next);
      },
      (loadError) => {
        if (!cancelled) setError(describeError(loadError));
      },
    );
    return () => {
      cancelled = true;
    };
  }, []);

  async function choose(choice: WindowSizeChoice) {
    setBusy(true);
    setError(null);
    try {
      setState(await api.setWindowSize(choice));
    } catch (saveError) {
      setError(describeError(saveError));
    } finally {
      setBusy(false);
    }
  }

  const options = state
    ? [
        ...state.presets.map((preset) => ({
          choice: preset.name,
          size: `${preset.width} × ${preset.height}`,
          fits: preset.fits,
        })),
        { choice: "free" as const, size: "any size", fits: true },
      ]
    : [];

  return (
    <PageSection
      title="Window"
      description="Vaultime opens at one fixed size, the way a game client does. Free lets you resize and maximize it."
    >
      {state && (
        <div role="radiogroup" aria-label="Window size">
          {options.map(({ choice, size, fits }) => {
            const checked = state.choice === choice;
            const hint = !fits
              ? "Too large for this screen."
              : choice === state.applied && choice !== state.choice
                ? "In use on this screen."
                : HINTS[choice];
            return (
              <label
                key={choice}
                className={cn(
                  "relative flex items-center gap-4 border-b border-rule py-3.5 first:pt-0 last:border-b-0",
                  fits && !busy ? "cursor-pointer" : "cursor-not-allowed",
                )}
              >
                <input
                  type="radio"
                  name="window-size"
                  value={choice}
                  checked={checked}
                  disabled={busy || !fits}
                  onChange={() => void choose(choice)}
                  className="peer sr-only"
                />
                <span
                  aria-hidden="true"
                  className={cn(
                    "flex size-[18px] shrink-0 items-center justify-center rounded-full border transition-colors peer-focus-visible:ring-2 peer-focus-visible:ring-violet/60",
                    checked ? "border-violet" : "border-hairline-strong",
                  )}
                >
                  {checked && <span className="size-2 rounded-full bg-violet" />}
                </span>
                <span className="min-w-0 flex-1">
                  <span className={cn("block text-[15px]", fits ? "text-text" : "text-faint")}>{NAMES[choice]}</span>
                  {hint && <span className="mt-0.5 block text-[13px] leading-relaxed text-faint">{hint}</span>}
                </span>
                <span
                  className={cn("shrink-0 font-mono text-sm", fits && choice !== "free" ? "text-soft" : "text-faint")}
                >
                  {size}
                </span>
              </label>
            );
          })}
        </div>
      )}
      {state && state.applied !== state.choice && (
        <Notice className="mt-5">
          {state.applied === "free"
            ? `${NAMES[state.choice]} and the sizes below it do not fit this screen, so the window is free for now.`
            : `${NAMES[state.choice]} does not fit this screen, so the window uses ${NAMES[state.applied]} for now.`}
        </Notice>
      )}
      {error && (
        <Notice tone="warning" className="mt-5">
          {error}
        </Notice>
      )}
    </PageSection>
  );
}
