// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { describe, expect, it } from "vitest";
import { COVER_ASPECT, CROP_MAX_ZOOM_OF_FILL, CROP_MIN_ZOOM_OF_FIT } from "./constants";
import {
  clampView,
  fillView,
  fitView,
  fitZoom,
  imagePlacement,
  initialView,
  panBy,
  sameView,
  toCrop,
  viewFromCrop,
  zoomAt,
  zoomBounds,
} from "./crop";

const wide = { width: 1600, height: 400 };
const tall = { width: 600, height: 1200 };
const close = (value: number, expected: number) => expect(value).toBeCloseTo(expected, 9);

describe("toCrop", () => {
  it("fills the cover with the middle of the image at zoom 1", () => {
    const crop = toCrop(fillView(wide), wide);
    // 400 high means a frame 300 wide in the middle of 1600.
    close(crop.width, 300 / 1600);
    close(crop.x, 650 / 1600);
    close(crop.y, 0);
    close(crop.height, 1);
  });

  it("keeps the cover aspect", () => {
    for (const size of [wide, tall]) {
      for (const view of [fillView(size), fitView(size), zoomAt(fillView(size), size, 2)]) {
        const crop = toCrop(view, size);
        close((crop.width * size.width) / (crop.height * size.height), COVER_ASPECT);
      }
    }
  });

  it("reaches past the edges when fitted", () => {
    const crop = toCrop(fitView(wide), wide);
    close(crop.x, 0);
    close(crop.width, 1);
    expect(crop.y).toBeLessThan(0);
    close(crop.y + crop.height / 2, 0.5);
  });

  it("round trips through a view", () => {
    const view = clampView({ zoom: 1.7, centerX: 500, centerY: 220 }, wide);
    expect(sameView(viewFromCrop(toCrop(view, wide), wide), view, wide)).toBe(true);
  });
});

describe("initialView", () => {
  it("fits wide images and fills tall ones", () => {
    expect(initialView(wide, null)).toEqual(fitView(wide));
    expect(initialView(tall, null)).toEqual(fillView(tall));
    expect(initialView({ width: 500, height: 500 }, null).zoom).toBeLessThan(1);
  });

  it("opens at the crop in use", () => {
    const crop = toCrop(clampView({ zoom: 2, centerX: 300, centerY: 200 }, wide), wide);
    expect(sameView(initialView(wide, crop), viewFromCrop(crop, wide), wide)).toBe(true);
  });
});

describe("clampView", () => {
  it("keeps the zoom in bounds", () => {
    const { min, max } = zoomBounds(wide);
    close(min, fitZoom(wide) * CROP_MIN_ZOOM_OF_FIT);
    expect(max).toBe(CROP_MAX_ZOOM_OF_FILL);
    expect(clampView({ zoom: 100, centerX: 800, centerY: 200 }, wide).zoom).toBe(max);
    expect(clampView({ zoom: 0, centerX: 800, centerY: 200 }, wide).zoom).toBe(min);
  });

  it("never zooms a tiny image to a frame narrower than one of its pixels", () => {
    const tiny = { width: 2, height: 3 };
    const { max } = zoomBounds(tiny);
    expect(max).toBe(2);
    expect(toCrop(clampView({ zoom: 100, centerX: 1, centerY: 1.5 }, tiny), tiny).width * tiny.width).toBeGreaterThanOrEqual(1);
  });

  it("keeps a filled frame inside the image", () => {
    const view = clampView({ zoom: 1, centerX: 0, centerY: 0 }, wide);
    const crop = toCrop(view, wide);
    close(crop.x, 0);
    close(crop.y, 0);
    const right = toCrop(clampView({ zoom: 1, centerX: 5000, centerY: 200 }, wide), wide);
    close(right.x + right.width, 1);
  });

  it("keeps a fitted image inside the frame", () => {
    const fitted = fitView(wide);
    // A frame far below the image puts the image at its top edge, no further.
    const atTop = toCrop(clampView({ ...fitted, centerY: 10_000 }, wide), wide);
    close(atTop.y, 0);
    const atBottom = toCrop(clampView({ ...fitted, centerY: -10_000 }, wide), wide);
    close(atBottom.y + atBottom.height, 1);
  });
});

describe("panBy", () => {
  it("moves the frame against the drag", () => {
    const zoomed = zoomAt(fillView(wide), wide, 2);
    const moved = panBy(zoomed, wide, { x: 0.25, y: 0 });
    const frameWidth = toCrop(zoomed, wide).width * wide.width;
    close(moved.centerX, zoomed.centerX - 0.25 * frameWidth);
  });
});

describe("zoomAt", () => {
  it("keeps the point under the pointer in place", () => {
    const start = clampView({ zoom: 1.5, centerX: 700, centerY: 200 }, wide);
    const anchor = { x: 0.2, y: 0.7 };
    const pointAt = (view: typeof start) => {
      const crop = toCrop(view, wide);
      return {
        x: (crop.x + anchor.x * crop.width) * wide.width,
        y: (crop.y + anchor.y * crop.height) * wide.height,
      };
    };
    const zoomed = zoomAt(start, wide, 1.3, anchor);
    close(pointAt(zoomed).x, pointAt(start).x);
    close(pointAt(zoomed).y, pointAt(start).y);
  });

  it("stops at the bounds", () => {
    expect(zoomAt(fillView(wide), wide, 1000).zoom).toBe(CROP_MAX_ZOOM_OF_FILL);
    expect(zoomAt(fillView(wide), wide, 0.0001).zoom).toBe(zoomBounds(wide).min);
  });
});

describe("imagePlacement", () => {
  it("draws the whole fitted image across the frame", () => {
    const place = imagePlacement(fitView(wide), wide, 300);
    close(place.left, 0);
    close(place.width, 300);
    close(place.height, 75);
    close(place.top, (400 - 75) / 2);
  });

  it("draws a filled image past the frame", () => {
    const place = imagePlacement(fillView(tall), tall, 300);
    close(place.width, 300);
    close(place.height, 600);
    close(place.top, -100);
  });
});
