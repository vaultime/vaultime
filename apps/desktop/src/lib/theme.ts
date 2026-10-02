// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

// The look the player picks: dark or light, a ground tone, an accent color
// and a background picture. The color tokens of index.css are derived from
// the first three, so every combination stays readable.

import {
  ACCENT_GREY_MAX_CHROMA,
  ACCENT_HOVER_CHROMA_SCALE,
  ACCENT_LEVELS,
  ACCENT_MIN_CHROMA,
  ACCENT_SWATCHES,
  BACKGROUND_BLUR_PX,
  BACKGROUND_DIM_PERCENT,
  GLINT_LIGHTNESS,
  GROUND_LEVELS,
  GROUNDS,
  MARK_LEVELS,
  MARK_NEUTRAL_LIGHTNESS,
  SCRIM_LIGHTNESS,
  SETTING_KEYS,
  SIGNAL_LEVELS,
  TINT_LEVELS,
} from "@/lib/constants";
import { formatOklch, hexToOklch, maxSrgbChroma, type Oklch } from "@/lib/color";

export type ThemeMode = "dark" | "light";
export type ModeChoice = ThemeMode | "system";
export type GroundId = keyof typeof GROUNDS;
export type AccentSwatch = keyof typeof ACCENT_SWATCHES;

export const MODE_CHOICES: ModeChoice[] = ["dark", "light", "system"];
export const GROUND_IDS = Object.keys(GROUNDS) as GroundId[];
export const ACCENT_SWATCH_IDS = Object.keys(ACCENT_SWATCHES) as AccentSwatch[];

export interface Appearance {
  mode: ModeChoice;
  ground: GroundId;
  /** A swatch name or a `#rrggbb` color the player picked. */
  accent: string;
  /** Share of the ground over the background picture, in percent. */
  dim: number;
  /** Blur of the background picture, in pixels. */
  blur: number;
}

export const DEFAULT_APPEARANCE: Appearance = {
  mode: "dark",
  ground: "vault",
  accent: "violet",
  dim: BACKGROUND_DIM_PERCENT.default,
  blur: BACKGROUND_BLUR_PX.default,
};

const HEX_COLOR = /^#[0-9a-f]{6}$/i;

function isSwatch(value: string): value is AccentSwatch {
  return Object.hasOwn(ACCENT_SWATCHES, value);
}

function isGround(value: string | undefined): value is GroundId {
  return value !== undefined && Object.hasOwn(GROUNDS, value);
}

function isAccent(value: string | undefined): value is string {
  return value !== undefined && (isSwatch(value) || HEX_COLOR.test(value));
}

function clampedNumber(value: string | undefined, range: { min: number; max: number; default: number }): number {
  const number = Number(value);
  if (value === undefined || value === "" || !Number.isFinite(number)) return range.default;
  return Math.min(range.max, Math.max(range.min, Math.round(number)));
}

/** The setting that stores each part of the look. */
export const APPEARANCE_SETTING_KEYS: Record<keyof Appearance, string> = {
  mode: SETTING_KEYS.appearanceMode,
  ground: SETTING_KEYS.appearanceGround,
  accent: SETTING_KEYS.appearanceAccent,
  dim: SETTING_KEYS.backgroundDim,
  blur: SETTING_KEYS.backgroundBlur,
};

/** The look from stored settings. Unknown or missing values take the default. */
export function appearanceFromSettings(values: Record<string, string | undefined>): Appearance {
  const mode = values[APPEARANCE_SETTING_KEYS.mode];
  const ground = values[APPEARANCE_SETTING_KEYS.ground];
  const accent = values[APPEARANCE_SETTING_KEYS.accent];
  return {
    mode: MODE_CHOICES.includes(mode as ModeChoice) ? (mode as ModeChoice) : DEFAULT_APPEARANCE.mode,
    ground: isGround(ground) ? ground : DEFAULT_APPEARANCE.ground,
    accent: isAccent(accent) ? accent.toLowerCase() : DEFAULT_APPEARANCE.accent,
    dim: clampedNumber(values[APPEARANCE_SETTING_KEYS.dim], BACKGROUND_DIM_PERCENT),
    blur: clampedNumber(values[APPEARANCE_SETTING_KEYS.blur], BACKGROUND_BLUR_PX),
  };
}

/** The stored value of each appearance setting. */
export function appearanceSettings(appearance: Partial<Appearance>): [string, string][] {
  return (Object.keys(appearance) as (keyof Appearance)[]).map((part) => [
    APPEARANCE_SETTING_KEYS[part],
    String(appearance[part]),
  ]);
}

export interface AccentColor extends Oklch {
  /** A grey accent has no hue to keep free. */
  grey: boolean;
}

/**
 * The accent at the lightness of the mode. The hue stays, chroma is cut to
 * what fits sRGB, and a picked grey stays grey.
 */
export function accentColor(accent: string, mode: ThemeMode): AccentColor {
  const levels = ACCENT_LEVELS[mode];
  const picked = isSwatch(accent)
    ? ACCENT_SWATCHES[accent]
    : (hexToOklch(accent) ?? ACCENT_SWATCHES[DEFAULT_APPEARANCE.accent as AccentSwatch]);
  if (picked.chroma < ACCENT_GREY_MAX_CHROMA) {
    return { lightness: levels.grey, chroma: 0, hue: 0, grey: true };
  }
  const chroma = Math.min(
    Math.max(picked.chroma, ACCENT_MIN_CHROMA),
    maxSrgbChroma(levels.lightness, picked.hue),
  );
  return { lightness: levels.lightness, chroma, hue: picked.hue, grey: false };
}

/** The hue journal marks keep free, none for a grey accent. */
export function accentHues(accent: string): number[] {
  const color = accentColor(accent, "dark");
  return color.grey ? [] : [color.hue];
}

/** The mode a choice stands for, given whether the system is dark. */
export function resolveMode(choice: ModeChoice, systemDark: boolean): ThemeMode {
  if (choice === "system") return systemDark ? "dark" : "light";
  return choice;
}

/** Every color token of index.css, plus the levels game tints and marks read. */
export function themeTokens(mode: ThemeMode, groundId: GroundId, accent: string): Record<string, string> {
  const ground = GROUNDS[groundId];
  const tokens: Record<string, string> = {};

  for (const [role, levels] of Object.entries(GROUND_LEVELS)) {
    const { lightness, chroma } = levels[mode];
    tokens[`--${role}`] = formatOklch({ lightness, chroma: chroma * ground.chroma, hue: ground.hue });
  }

  const levels = ACCENT_LEVELS[mode];
  const color = accentColor(accent, mode);
  tokens["--violet"] = formatOklch(color);
  tokens["--violet-hover"] = formatOklch({
    lightness: color.grey ? levels.greyHover : levels.hover,
    chroma: Math.min(color.chroma * ACCENT_HOVER_CHROMA_SCALE, maxSrgbChroma(levels.hover, color.hue)),
    hue: color.hue,
  });
  tokens["--violet-ink"] = formatOklch({
    lightness: levels.ink,
    chroma: color.grey ? 0 : levels.inkChroma,
    hue: color.hue,
  });
  // Tints of pages that are not about one game take the accent's hue, a grey
  // accent the ground's.
  tokens["--accent-hue"] = String(Math.round(color.grey ? ground.hue : color.hue));

  for (const [name, signal] of Object.entries(SIGNAL_LEVELS)) {
    tokens[`--${name}`] = formatOklch(signal[mode]);
  }
  tokens["--scrim"] = formatOklch({ lightness: SCRIM_LIGHTNESS[mode], chroma: 0, hue: 0 });
  tokens["--glint"] = formatOklch({ lightness: GLINT_LIGHTNESS[mode], chroma: 0, hue: 0 });

  for (const [role, level] of Object.entries(TINT_LEVELS)) {
    tokens[`--tint-${role}`] = String(level[mode]);
  }
  tokens["--mark-lightness"] = String(MARK_LEVELS.lightness[mode]);
  for (const [index, lightness] of MARK_NEUTRAL_LIGHTNESS[mode].entries()) {
    tokens[`--mark-grey-${index + 1}`] = String(lightness);
  }
  return tokens;
}

/** The ground and text colors of a combination, for previews and tests. */
export function groundColor(mode: ThemeMode, groundId: GroundId, role: keyof typeof GROUND_LEVELS): Oklch {
  const ground = GROUNDS[groundId];
  const { lightness, chroma } = GROUND_LEVELS[role][mode];
  return { lightness, chroma: chroma * ground.chroma, hue: ground.hue };
}
