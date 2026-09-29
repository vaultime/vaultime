// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config.ts";

// Every test runs in several time zones, so code that mixes up local and UTC
// days fails somewhere. West and east of UTC, a half hour offset, the date
// line and UTC itself.
const TIME_ZONES = [
  "Europe/Berlin",
  "America/Los_Angeles",
  "Asia/Kolkata",
  "Pacific/Auckland",
  "America/Sao_Paulo",
  "UTC",
];

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      projects: TIME_ZONES.map((zone) => ({
        extends: true,
        test: {
          name: zone,
          include: ["src/**/*.test.ts"],
          setupFiles: ["src/test/setup.ts"],
          env: { TZ: zone },
        },
      })),
    },
  }),
);
