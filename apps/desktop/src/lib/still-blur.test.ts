// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { afterEach, describe, expect, it, vi } from "vitest";
import { STILL_BLUR_STORAGE_KEY } from "@/lib/constants";
import { coverRect, stillBlurSize } from "./still-blur";

describe("stillBlurSize", () => {
  it("draws a strong blur small, a few of its pixels wide", () => {
    const size = stillBlurSize({ width: 1440, height: 900 }, 44, 1);
    expect([size.width, size.height]).toEqual([65, 41]);
    expect(size.radius).toBeCloseTo(2);
  });

  it("stays sharp at the screen's pixels without a blur", () => {
    expect(stillBlurSize({ width: 1440, height: 900 }, 0, 2)).toEqual({ width: 2880, height: 1800, radius: 0 });
  });

  it("never draws larger than the box for a faint blur", () => {
    expect(stillBlurSize({ width: 300, height: 200 }, 1, 2)).toEqual({ width: 300, height: 200, radius: 1 });
  });

  it("keeps at least one pixel for a box too small to see", () => {
    const size = stillBlurSize({ width: 3, height: 3 }, 64, 1);
    expect([size.width, size.height]).toEqual([1, 1]);
  });
});

describe("coverRect", () => {
  it("covers the box and reaches past each edge, keeping the image's shape", () => {
    const rect = coverRect({ width: 65, height: 41 }, { width: 600, height: 900 }, 4);
    // Four pixels past the left and right edges, the height cut as object-cover does.
    expect(rect.x).toBe(-4);
    expect(rect.width).toBe(73);
    expect(rect.height / rect.width).toBeCloseTo(900 / 600);
    expect(rect.y).toBeCloseTo((41 - rect.height) / 2);
    expect(rect.y + rect.height).toBeGreaterThan(41 + 4);
  });

  it("lays a wide image past the top and bottom", () => {
    const rect = coverRect({ width: 40, height: 40 }, { width: 1920, height: 1080 }, 4);
    expect(rect.y).toBe(-4);
    expect(rect.x).toBeLessThan(-4);
  });

  it("fills the box exactly without a margin", () => {
    expect(coverRect({ width: 2880, height: 1800 }, { width: 1440, height: 900 }, 0)).toEqual({
      x: 0,
      y: 0,
      width: 2880,
      height: 1800,
    });
  });
});

describe("stillBlur", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  async function switchWith(getItem: (key: string) => string | null) {
    vi.stubGlobal("localStorage", { getItem });
    vi.resetModules();
    return (await import("./still-blur")).stillBlur;
  }

  it("is on only when the switch says still, and read once", async () => {
    expect((await switchWith(() => null))()).toBe(false);
    expect((await switchWith(() => "live"))()).toBe(false);
    let saved: string | null = "still";
    const stillBlur = await switchWith((key) => (key === STILL_BLUR_STORAGE_KEY ? saved : null));
    expect(stillBlur()).toBe(true);
    saved = null;
    expect(stillBlur()).toBe(true);
  });

  it("is off where storage cannot be read", async () => {
    const stillBlur = await switchWith(() => {
      throw new Error("blocked");
    });
    expect(stillBlur()).toBe(false);
  });
});
