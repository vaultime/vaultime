// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { contrastRatio, hexToOklch, oklchToHex, type Oklch } from "./color";
import { ACCENT_MIN_CHROMA, BACKGROUND_BLUR_PX, BACKGROUND_DIM_PERCENT, SETTING_KEYS, TINT_LEVELS } from "./constants";
import { markColors } from "./game-tint";
import {
  ACCENT_SWATCH_IDS,
  DEFAULT_APPEARANCE,
  GROUND_IDS,
  accentColor,
  accentHues,
  appearanceFromSettings,
  appearanceSettings,
  resolveMode,
  themeTokens,
  type ThemeMode,
} from "./theme";

const MODES: ThemeMode[] = ["dark", "light"];

/** Every mode, ground and swatch accent. */
const COMBINATIONS = MODES.flatMap((mode) =>
  GROUND_IDS.flatMap((ground) => ACCENT_SWATCH_IDS.map((accent) => ({ mode, ground, accent }))),
);

/** Parses `oklch(l c h)`, with tokens put in. */
function parse(color: string, tokens: Record<string, string> = {}): Oklch {
  const resolved = color.replace(/var\((--[a-z0-9-]+)\)/g, (_, name: string) => tokens[name]);
  const [lightness, chroma, hue] = resolved.slice("oklch(".length, -1).split(" ").map(Number);
  return { lightness, chroma, hue };
}

describe("themeTokens", () => {
  it("keeps the dark violet look of Vaultime 0.2 as the default", () => {
    const tokens = themeTokens("dark", "vault", "violet");
    const before: Record<string, string> = {
      "--ink": "#0e0b14",
      "--surface": "#16121e",
      "--raised": "#1a1524",
      "--hairline": "#2a2238",
      "--rule": "#231c31",
      "--text": "#eee8f6",
      "--soft": "#d9d1e6",
      "--faint": "#a99fbc",
      "--violet": "#9d7cff",
      "--violet-hover": "#b39aff",
      "--violet-ink": "#140e22",
      "--hairline-strong": "#3a3050",
      "--bar": "#120e19",
      "--idle": "#4a4060",
      "--amber": "#f0b45a",
      "--sky": "#7fb0ff",
      "--destructive": "#ff7a85",
    };
    for (const [token, hex] of Object.entries(before)) {
      const derived = oklchToHex(parse(tokens[token]));
      const channels = (value: string) => [1, 3, 5].map((start) => parseInt(value.slice(start, start + 2), 16));
      const largest = Math.max(...channels(derived).map((channel, index) => Math.abs(channel - channels(hex)[index])));
      expect(largest, `${token} ${derived} vs ${hex}`).toBeLessThanOrEqual(3);
    }
  });

  it.each(COMBINATIONS)("reads well in $mode mode on $ground with $accent", ({ mode, ground, accent }) => {
    const tokens = themeTokens(mode, ground, accent);
    const color = (token: string) => parse(tokens[token]);
    for (const field of ["--ink", "--bar", "--surface"]) {
      expect(contrastRatio(color("--text"), color(field))).toBeGreaterThanOrEqual(7);
      expect(contrastRatio(color("--soft"), color(field))).toBeGreaterThanOrEqual(7);
      expect(contrastRatio(color("--faint"), color(field))).toBeGreaterThanOrEqual(4.5);
      expect(contrastRatio(color("--violet"), color(field))).toBeGreaterThanOrEqual(4.5);
      for (const signal of ["--amber", "--sky", "--destructive"]) {
        expect(contrastRatio(color(signal), color(field)), signal).toBeGreaterThanOrEqual(4.5);
      }
    }
    expect(contrastRatio(color("--faint"), color("--raised"))).toBeGreaterThanOrEqual(4.5);
    expect(contrastRatio(color("--violet-ink"), color("--violet"))).toBeGreaterThanOrEqual(4.5);
    // Rules and idle time stay visible against the ground.
    expect(contrastRatio(color("--hairline"), color("--ink"))).toBeGreaterThan(1.2);
    expect(contrastRatio(color("--idle"), color("--ink"))).toBeGreaterThanOrEqual(1.5);
    expect(contrastRatio(color("--violet"), color("--idle"))).toBeGreaterThanOrEqual(1.5);
  });

  it.each(MODES)("gives game tints readable text in %s mode", (mode) => {
    const tokens = themeTokens(mode, "vault", "violet");
    for (let hue = 0; hue < 360; hue += 30) {
      const tint = (role: keyof typeof TINT_LEVELS) =>
        parse(`oklch(var(--tint-${role}) ${TINT_LEVELS[role].chroma} ${hue})`, tokens);
      for (const field of [tint("fill"), tint("wash")]) {
        expect(contrastRatio(tint("ink"), field)).toBeGreaterThanOrEqual(4.5);
        expect(contrastRatio(tint("soft"), field)).toBeGreaterThanOrEqual(4.5);
      }
      expect(contrastRatio(tint("muted"), tint("wash"))).toBeGreaterThanOrEqual(4.5);
    }
  });

  it.each(MODES)("keeps journal marks apart from the ground in %s mode", (mode) => {
    const tokens = themeTokens(mode, "vault", "violet");
    const colors = Array.from({ length: 12 }, (_, index) => ({ hue: index * 30, chroma: 0.15 }));
    for (const mark of [...markColors(colors), ...markColors([null, null, null])]) {
      expect(contrastRatio(parse(mark, tokens), parse(tokens["--ink"]))).toBeGreaterThanOrEqual(3);
    }
  });

  it("sets the color scheme tokens for both modes", () => {
    const dark = themeTokens("dark", "graphite", "teal");
    const light = themeTokens("light", "graphite", "teal");
    expect(Object.keys(dark).sort()).toEqual(Object.keys(light).sort());
    expect(parse(light["--ink"]).lightness).toBeGreaterThan(parse(dark["--ink"]).lightness);
    expect(parse(light["--text"]).lightness).toBeLessThan(parse(dark["--text"]).lightness);
  });
});

describe("accentColor", () => {
  it("brings every color to the lightness of the mode", () => {
    for (const hex of ["#ff0000", "#204060", "#ffee88", "#00ff88"]) {
      for (const mode of MODES) {
        const picked = accentColor(hex, mode);
        const swatch = accentColor("violet", mode);
        expect(picked.lightness).toBe(swatch.lightness);
        expect(Math.abs(picked.hue - hexToOklch(hex)!.hue)).toBeLessThan(1);
      }
    }
  });

  it("keeps a picked grey grey", () => {
    for (const hex of ["#000000", "#ffffff", "#808080"]) {
      const color = accentColor(hex, "dark");
      expect(color.grey).toBe(true);
      expect(color.chroma).toBe(0);
    }
    expect(accentHues("#808080")).toEqual([]);
  });

  it("gives a muted color enough chroma to read as a color", () => {
    expect(accentColor("#6b5f73", "dark").chroma).toBeGreaterThanOrEqual(ACCENT_MIN_CHROMA);
  });

  it("falls back to violet for an unknown value", () => {
    expect(accentColor("nonsense", "dark")).toEqual(accentColor("violet", "dark"));
  });
});

describe("appearanceFromSettings", () => {
  it("takes defaults for missing and unknown values", () => {
    expect(appearanceFromSettings({})).toEqual(DEFAULT_APPEARANCE);
    expect(
      appearanceFromSettings({
        [SETTING_KEYS.appearanceMode]: "sepia",
        [SETTING_KEYS.appearanceGround]: "toString",
        [SETTING_KEYS.appearanceAccent]: "#12345",
        [SETTING_KEYS.backgroundDim]: "much",
      }),
    ).toEqual(DEFAULT_APPEARANCE);
  });

  it("reads stored values and keeps numbers in range", () => {
    const appearance = appearanceFromSettings({
      [SETTING_KEYS.appearanceMode]: "light",
      [SETTING_KEYS.appearanceGround]: "umber",
      [SETTING_KEYS.appearanceAccent]: "#AA3355",
      [SETTING_KEYS.backgroundDim]: "5",
      [SETTING_KEYS.backgroundBlur]: "999",
    });
    expect(appearance).toEqual({
      mode: "light",
      ground: "umber",
      accent: "#aa3355",
      dim: BACKGROUND_DIM_PERCENT.min,
      blur: BACKGROUND_BLUR_PX.max,
    });
    expect(appearanceFromSettings(Object.fromEntries(appearanceSettings(appearance)))).toEqual(appearance);
  });
});

describe("resolveMode", () => {
  it("follows the system only when asked to", () => {
    expect(resolveMode("system", true)).toBe("dark");
    expect(resolveMode("system", false)).toBe("light");
    expect(resolveMode("light", true)).toBe("light");
    expect(resolveMode("dark", false)).toBe("dark");
  });
});
