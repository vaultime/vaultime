// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { act, render } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { StillBlur } from "./StillBlur";

/** The media queries asked for, each with the listeners it holds now. */
const queries: { media: string; listeners: Set<() => void> }[] = [];

function sized(prototype: object, sizes: Record<string, number>) {
  for (const [name, value] of Object.entries(sizes)) {
    Object.defineProperty(prototype, name, { configurable: true, get: () => value });
  }
}

function setScale(ratio: number) {
  Object.defineProperty(window, "devicePixelRatio", { configurable: true, value: ratio });
}

beforeEach(() => {
  queries.length = 0;
  // happy-dom lays nothing out and decodes no pictures, so the canvas and the
  // picture get sizes, and a resize observer reports once at the start, as a
  // browser's does.
  sized(HTMLCanvasElement.prototype, { clientWidth: 100, clientHeight: 50 });
  sized(HTMLImageElement.prototype, { naturalWidth: 400, naturalHeight: 200 });
  vi.spyOn(HTMLImageElement.prototype, "decode").mockImplementation(function (this: HTMLImageElement) {
    return this.src.includes("missing") ? Promise.reject(new Error("gone")) : Promise.resolve();
  });
  vi.stubGlobal(
    "ResizeObserver",
    class {
      private readonly report: () => void;
      constructor(report: () => void) {
        this.report = report;
      }
      observe() {
        this.report();
      }
      disconnect() {}
    },
  );
  vi.stubGlobal("matchMedia", (media: string) => {
    const query = { media, listeners: new Set<() => void>() };
    queries.push(query);
    return {
      media,
      addEventListener: (_type: string, listener: () => void) => query.listeners.add(listener),
      removeEventListener: (_type: string, listener: () => void) => query.listeners.delete(listener),
    };
  });
  setScale(1);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
  for (const name of ["clientWidth", "clientHeight"]) Reflect.deleteProperty(HTMLCanvasElement.prototype, name);
  for (const name of ["naturalWidth", "naturalHeight"]) Reflect.deleteProperty(HTMLImageElement.prototype, name);
});

describe("StillBlur", () => {
  it("draws a picture without a blur again at the scale of the screen it moved to", async () => {
    const { container } = render(<StillBlur src="/art.jpg" blur={0} />);
    await act(async () => {});
    const canvas = container.querySelector("canvas")!;
    expect(canvas.width).toBe(100);
    expect(queries.at(-1)?.media).toBe("(resolution: 1dppx)");

    setScale(2);
    const moved = queries.at(-1)!;
    act(() => moved.listeners.forEach((listener) => listener()));
    expect(canvas.width).toBe(200);
    // The old scale is let go and the new one watched.
    expect(moved.listeners.size).toBe(0);
    expect(queries.at(-1)?.media).toBe("(resolution: 2dppx)");
    expect(queries.at(-1)?.listeners.size).toBe(1);
  });

  it("watches no scale with a blur, as its canvas is smaller than the screen's pixels anyway", async () => {
    const { container } = render(<StillBlur src="/art.jpg" blur={20} />);
    await act(async () => {});
    // Two pixels of blur in the small canvas stand for the 20 of the window.
    expect(container.querySelector("canvas")!.width).toBe(10);
    expect(queries).toHaveLength(0);
  });

  it("shows no picture for one that does not load, not the one before it", async () => {
    const { container, rerender } = render(<StillBlur src="/art.jpg" blur={0} />);
    await act(async () => {});
    const canvas = container.querySelector("canvas")!;
    expect(canvas.width).toBe(100);

    rerender(<StillBlur src="/missing.jpg" blur={0} />);
    await act(async () => {});
    expect(canvas.width).toBe(0);
  });
});
