// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import boot from "../../public/theme-boot.js?raw";
import { APPEARANCE_CACHE_KEY } from "./constants";

/** Runs the boot script against a stored copy and a system mode, returns what it put on the page. */
function runBoot(stored: unknown, systemDark: boolean) {
  const properties = new Map<string, string>();
  const classes = new Set<string>();
  const root = {
    style: {
      colorScheme: "",
      setProperty: (name: string, value: string) => properties.set(name, value),
    },
    classList: { toggle: (name: string, on: boolean) => (on ? classes.add(name) : classes.delete(name)) },
  };
  const storage = {
    getItem: (key: string) => (key === APPEARANCE_CACHE_KEY && stored !== undefined ? JSON.stringify(stored) : null),
  };
  const matchMedia = () => ({ matches: systemDark });
  new Function("localStorage", "document", "matchMedia", boot)(storage, { documentElement: root }, matchMedia);
  return { properties, scheme: root.style.colorScheme, dark: classes.has("dark") };
}

const tokens = { dark: { "--ink": "dark ink" }, light: { "--ink": "light ink" } };

describe("theme-boot.js", () => {
  it("puts the cached tokens of the chosen mode on the page", () => {
    const page = runBoot({ appearance: { mode: "light" }, tokens }, true);
    expect(page.properties.get("--ink")).toBe("light ink");
    expect(page.scheme).toBe("light");
    expect(page.dark).toBe(false);
  });

  it("follows the system in System mode", () => {
    expect(runBoot({ appearance: { mode: "system" }, tokens }, true).properties.get("--ink")).toBe("dark ink");
    const light = runBoot({ appearance: { mode: "system" }, tokens }, false);
    expect(light.properties.get("--ink")).toBe("light ink");
    expect(light.scheme).toBe("light");
  });

  it("starts in the default look without a copy, or with one in the old format", () => {
    expect(runBoot(undefined, true).properties.size).toBe(0);
    const old = runBoot({ appearance: { mode: "dark" }, mode: "dark", tokens: { "--ink": "old ink" } }, true);
    expect(old.properties.size).toBe(0);
  });

  it("only sets custom properties", () => {
    const page = runBoot({ appearance: { mode: "dark" }, tokens: { dark: { color: "red", "--ink": "x" } } }, true);
    expect([...page.properties.keys()]).toEqual(["--ink"]);
  });
});
