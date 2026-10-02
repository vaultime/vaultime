// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { Slider as SliderPrimitive } from "@base-ui/react/slider";
import { cn } from "@/lib/utils";

/** A hairline track with the chosen part in the accent and a small round thumb. */
export function Slider({
  value,
  min,
  max,
  step = 1,
  label,
  valueText,
  onChange,
  className,
}: {
  value: number;
  min: number;
  max: number;
  step?: number;
  /** Name of the value for screen readers. */
  label: string;
  /** How screen readers read the value, like "78 percent". */
  valueText: (value: number) => string;
  onChange: (value: number) => void;
  className?: string;
}) {
  return (
    <SliderPrimitive.Root
      value={value}
      min={min}
      max={max}
      step={step}
      onValueChange={(next) => onChange(next)}
      className={cn("w-full touch-none select-none", className)}
    >
      <SliderPrimitive.Control className="flex h-6 w-full cursor-pointer items-center">
        <SliderPrimitive.Track className="relative h-[3px] w-full rounded-full bg-hairline">
          <SliderPrimitive.Indicator className="rounded-full bg-violet" />
          <SliderPrimitive.Thumb
            aria-label={label}
            getAriaValueText={(_, current) => valueText(current)}
            className="size-3.5 rounded-full bg-violet ring-4 ring-ink outline-none focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-violet/70"
          />
        </SliderPrimitive.Track>
      </SliderPrimitive.Control>
    </SliderPrimitive.Root>
  );
}
