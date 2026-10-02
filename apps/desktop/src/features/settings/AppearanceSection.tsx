// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState, type ReactNode } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Check, ImagePlus, Images, Loader2, Pipette, X } from "lucide-react";
import { PageSection } from "@/components/layout/Page";
import { Cover } from "@/components/media/Cover";
import { Button } from "@/components/ui/button";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Slider } from "@/components/ui/slider";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useLibrary } from "@/features/library/library-context";
import { formatOklch, oklchToHex } from "@/lib/color";
import { ACCENT_LEVELS, ARTWORK_EXTENSIONS, BACKGROUND_BLUR_PX, BACKGROUND_DIM_PERCENT } from "@/lib/constants";
import {
  ACCENT_SWATCH_IDS,
  GROUND_IDS,
  accentColor,
  groundColor,
  type AccentSwatch,
  type GroundId,
  type ModeChoice,
} from "@/lib/theme";
import { cn, describeError } from "@/lib/utils";

const MODES: { value: ModeChoice; label: string }[] = [
  { value: "dark", label: "Dark" },
  { value: "light", label: "Light" },
  { value: "system", label: "System" },
];

const GROUND_NAMES: Record<GroundId, string> = {
  vault: "Vault",
  graphite: "Graphite",
  midnight: "Midnight",
  moss: "Moss",
  umber: "Umber",
};

const ACCENT_NAMES: Record<AccentSwatch, string> = {
  violet: "Violet",
  blue: "Blue",
  teal: "Teal",
  green: "Green",
  orange: "Orange",
  rose: "Rose",
};

/** Hues around the wheel for the swatch that opens the color picker. */
const WHEEL_STEPS = 6;

/** A label and hint with the controls below, for choices too wide to sit beside them. */
function StackedRow({ label, hint, children }: { label: string; hint?: ReactNode; children: ReactNode }) {
  return (
    <div className="border-b border-rule py-5 first:pt-0 last:border-b-0">
      <div className="text-[15px] text-text">{label}</div>
      {hint && <p className="mt-1 max-w-[520px] text-[13px] leading-relaxed text-faint">{hint}</p>}
      <div className="mt-4">{children}</div>
    </div>
  );
}

/** Dark or light, a ground tone, an accent and a background picture. */
export function AppearanceSection() {
  const { appearance, mode, background, change, chooseBackground, pickGameBackground, clearBackground } =
    useAppearance();
  const { summaries } = useLibrary();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const withArt = summaries.filter((summary) => summary.cover);
  const custom = appearance.accent.startsWith("#");
  const accentLightness = ACCENT_LEVELS[mode].lightness;
  const wheel = Array.from({ length: WHEEL_STEPS + 1 }, (_, step) =>
    formatOklch({ lightness: accentLightness, chroma: accentColor("violet", mode).chroma, hue: (step * 360) / WHEEL_STEPS }),
  );

  async function run(task: () => Promise<void>) {
    try {
      setBusy(true);
      setError(null);
      await task();
    } catch (taskError) {
      setError(describeError(taskError));
    } finally {
      setBusy(false);
    }
  }

  const choosePicture = () =>
    run(async () => {
      const selected = await openFileDialog({
        multiple: false,
        directory: false,
        title: "Choose a background picture",
        filters: [{ name: "Pictures", extensions: ARTWORK_EXTENSIONS }],
      });
      if (typeof selected === "string") await chooseBackground(selected);
    });

  return (
    <PageSection
      title="Appearance"
      description="How Vaultime looks on this PC. Backups leave the look and the picture out."
    >
      <StackedRow label="Mode" hint="System follows the dark or light setting of your PC.">
        <div role="radiogroup" aria-label="Mode" className="inline-flex rounded-full border border-hairline p-0.5">
          {MODES.map((option) => (
            <button
              key={option.value}
              type="button"
              role="radio"
              aria-checked={appearance.mode === option.value}
              onClick={() => change({ mode: option.value })}
              className={cn(
                "h-8 rounded-full px-4 text-[13px] transition-colors focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none",
                appearance.mode === option.value ? "bg-raised text-text" : "text-faint hover:text-soft",
              )}
            >
              {option.label}
            </button>
          ))}
        </div>
      </StackedRow>

      <StackedRow label="Ground" hint="The tone of the pages behind the text.">
        <div role="radiogroup" aria-label="Ground" className="flex flex-wrap gap-4">
          {GROUND_IDS.map((ground) => {
            const tone = (role: Parameters<typeof groundColor>[2]) => formatOklch(groundColor(mode, ground, role));
            const selected = appearance.ground === ground;
            return (
              <button
                key={ground}
                type="button"
                role="radio"
                aria-checked={selected}
                onClick={() => change({ ground })}
                className="group flex flex-col items-start gap-2 rounded-md focus-visible:outline-none"
              >
                <span
                  className={cn(
                    "flex h-14 w-[88px] flex-col justify-end gap-1.5 rounded-md border p-2.5 transition-shadow group-focus-visible:ring-2 group-focus-visible:ring-violet/70",
                    selected && "ring-2 ring-violet ring-offset-2 ring-offset-ink",
                  )}
                  style={{ background: tone("ink"), borderColor: tone("hairline") }}
                >
                  <span className="h-[3px] w-10 rounded-full" style={{ background: tone("text") }} />
                  <span className="flex items-center gap-1.5">
                    <span className="h-[3px] w-6 rounded-full" style={{ background: tone("faint") }} />
                    <span className="size-1.5 rounded-full bg-violet" />
                  </span>
                </span>
                <span className={cn("text-[13px]", selected ? "text-text" : "text-faint group-hover:text-soft")}>
                  {GROUND_NAMES[ground]}
                </span>
              </button>
            );
          })}
        </div>
      </StackedRow>

      <StackedRow label="Accent" hint="The color of active time, buttons and highlights. Any color you pick is brought to a lightness that reads well.">
        <div role="radiogroup" aria-label="Accent" className="flex flex-wrap items-center gap-3">
          {ACCENT_SWATCH_IDS.map((swatch) => {
            const selected = appearance.accent === swatch;
            return (
              <button
                key={swatch}
                type="button"
                role="radio"
                aria-checked={selected}
                aria-label={ACCENT_NAMES[swatch]}
                title={ACCENT_NAMES[swatch]}
                onClick={() => change({ accent: swatch })}
                className={cn(
                  "flex size-8 items-center justify-center rounded-full transition-shadow focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:ring-offset-2 focus-visible:ring-offset-ink focus-visible:outline-none",
                  selected && "ring-2 ring-text ring-offset-2 ring-offset-ink",
                )}
                style={{ background: formatOklch(accentColor(swatch, mode)) }}
              >
                {selected && <Check className="size-4 text-violet-ink" strokeWidth={2.4} />}
              </button>
            );
          })}
          <label
            title="Your own color"
            className={cn(
              "relative flex size-8 cursor-pointer items-center justify-center rounded-full focus-within:ring-2 focus-within:ring-violet/70 focus-within:ring-offset-2 focus-within:ring-offset-ink",
              custom && "ring-2 ring-text ring-offset-2 ring-offset-ink",
            )}
            style={{
              background: custom ? formatOklch(accentColor(appearance.accent, mode)) : `conic-gradient(${wheel.join(", ")})`,
            }}
          >
            <input
              type="color"
              aria-label="Your own color"
              value={custom ? appearance.accent : oklchToHex(accentColor(appearance.accent, mode))}
              onChange={(event) => change({ accent: event.target.value.toLowerCase() })}
              className="absolute inset-0 size-full cursor-pointer opacity-0"
            />
            {custom ? (
              <Check className="pointer-events-none size-4 text-violet-ink" strokeWidth={2.4} />
            ) : (
              <Pipette className="pointer-events-none size-3.5 text-ink" strokeWidth={2} />
            )}
          </label>
          {custom && <span className="ml-1 font-mono text-[13px] text-faint">{appearance.accent}</span>}
        </div>
      </StackedRow>

      <StackedRow
        label="Background picture"
        hint="A picture of your own or the art of a game, behind every page. It stays on this PC."
      >
        <div className="flex flex-wrap items-center gap-5">
          {background && (
            <span className="relative h-[90px] w-[144px] overflow-hidden rounded-md border border-hairline">
              <img src={background} alt="" className="size-full object-cover" />
              <span className="absolute inset-0 bg-ink" style={{ opacity: appearance.dim / 100 }} />
              <span className="absolute inset-x-3 bottom-3 h-[3px] w-12 rounded-full bg-text" />
            </span>
          )}
          <div className="flex flex-wrap gap-2">
            <Button variant="outline" size="sm" onClick={() => void choosePicture()} disabled={busy}>
              {busy ? <Loader2 className="size-3.5 animate-spin" /> : <ImagePlus className="size-3.5" />}
              Choose a picture
            </Button>
            <DropdownMenu>
              <DropdownMenuTrigger
                render={
                  <Button variant="outline" size="sm" disabled={busy || withArt.length === 0}>
                    <Images className="size-3.5" />
                    Use a game's art
                  </Button>
                }
              />
              <DropdownMenuContent align="start" className="w-64">
                {withArt.map((summary) => (
                  <DropdownMenuItem
                    key={summary.game.id}
                    onClick={() => void run(() => pickGameBackground(summary.game.id))}
                  >
                    <Cover title={summary.game.title} src={summary.cover} variant="tile" className="size-7 text-xs" />
                    <span className="truncate">{summary.game.title}</span>
                  </DropdownMenuItem>
                ))}
              </DropdownMenuContent>
            </DropdownMenu>
            {background && (
              <Button variant="ghost" size="sm" onClick={() => void run(clearBackground)} disabled={busy}>
                <X className="size-3.5" />
                Remove
              </Button>
            )}
          </div>
        </div>

        {background && (
          <div className="mt-6 grid max-w-[520px] gap-5">
            <SliderRow
              label="Dim"
              value={`${appearance.dim}%`}
              hint="How much of the ground lies over the picture."
            >
              <Slider
                label="Dim"
                value={appearance.dim}
                min={BACKGROUND_DIM_PERCENT.min}
                max={BACKGROUND_DIM_PERCENT.max}
                valueText={(value) => `${value} percent`}
                onChange={(dim) => change({ dim })}
              />
            </SliderRow>
            <SliderRow label="Blur" value={`${appearance.blur} px`} hint="Softens the picture so text stands out.">
              <Slider
                label="Blur"
                value={appearance.blur}
                min={BACKGROUND_BLUR_PX.min}
                max={BACKGROUND_BLUR_PX.max}
                valueText={(value) => `${value} pixels`}
                onChange={(blur) => change({ blur })}
              />
            </SliderRow>
          </div>
        )}

        {error && (
          <p role="alert" className="mt-3 text-[13px] text-amber">
            {error}
          </p>
        )}
      </StackedRow>
    </PageSection>
  );
}

function SliderRow({ label, value, hint, children }: { label: string; value: string; hint: string; children: ReactNode }) {
  return (
    <div>
      <div className="flex items-baseline justify-between gap-4">
        <span className="label-caps">{label}</span>
        <span className="font-mono text-[13px] text-soft tabular-nums">{value}</span>
      </div>
      <div className="mt-1.5">{children}</div>
      <p className="mt-1 text-[12px] text-faint">{hint}</p>
    </div>
  );
}
