import { defineConfig, mergeConfig } from "vitest/config";
import viteConfig from "./vite.config.ts";

// Tests see dates the way a user in Berlin does, so day boundaries differ
// from UTC and bugs that mix the two show up.
process.env.TZ = "Europe/Berlin";

export default mergeConfig(
  viteConfig,
  defineConfig({
    test: {
      include: ["src/**/*.test.ts"],
      setupFiles: ["src/test/setup.ts"],
    },
  }),
);
