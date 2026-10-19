// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { defineConfig } from "vitest/config";
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

// Only the path aliases come from the app's config, not its React and
// Tailwind plugins. Test files of one time zone share their modules to start
// faster, so a test must not leave stubs or other global state behind.
// Component tests (*.test.tsx) render pages in a browser-like document, once,
// each file with modules of its own, as they mock the core's commands.
export default defineConfig({
  resolve: viteConfig.resolve,
  test: {
    isolate: false,
    projects: [
      ...TIME_ZONES.map((zone) => ({
        extends: true,
        test: {
          name: zone,
          include: ["src/**/*.test.ts"],
          setupFiles: ["src/test/setup.ts"],
          env: { TZ: zone },
        },
      })),
      {
        extends: true,
        test: {
          name: "components",
          include: ["src/**/*.test.tsx"],
          environment: "happy-dom",
          isolate: true,
          setupFiles: ["src/test/setup-dom.ts"],
          env: { TZ: TIME_ZONES[0] },
        },
      },
    ],
  },
});
