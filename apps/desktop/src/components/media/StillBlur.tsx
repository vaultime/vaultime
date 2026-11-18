// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useRef, useState } from "react";
import { BACKGROUND_BLUR_BLEED, STILL_BLUR_BOX_PASSES } from "@/lib/constants";
import { coverRect, stillBlurSize } from "@/lib/still-blur";

// The webview of Linux has no canvas filters.
const canFilter = typeof CanvasRenderingContext2D !== "undefined" && "filter" in CanvasRenderingContext2D.prototype;

/**
 * An image blurred once into a small canvas, which the window scales up. A
 * live blur filter is worked out again with every repaint on a computer
 * without a graphics chip. Fills its box the way `object-cover` does.
 */
export function StillBlur({
  src,
  blur,
  saturation = 1,
  className,
}: {
  src: string;
  /** In CSS pixels of the box. */
  blur: number;
  /** How much more colorful the image is drawn, 1 as it is. */
  saturation?: number;
  className?: string;
}) {
  const canvas = useRef<HTMLCanvasElement>(null);
  // Decoded once per picture, so a move of the blur slider only draws again.
  const [image, setImage] = useState<HTMLImageElement | null>(null);

  useEffect(() => {
    let cancelled = false;
    const loading = new Image();
    loading.decoding = "async";
    loading.src = src;
    loading.decode().then(
      () => {
        if (!cancelled) setImage(loading);
      },
      () => {
        // A picture that does not load shows none, not the one before it.
        if (!cancelled) setImage(null);
      },
    );
    return () => {
      cancelled = true;
    };
  }, [src]);

  useEffect(() => {
    const target = canvas.current;
    if (!target) return;
    if (!image) {
      target.width = 0;
      return;
    }
    let cancelled = false;
    const paint = () => {
      if (!cancelled) draw(target, image, blur, saturation);
    };
    // Its first call comes with the size the canvas has now.
    const resized = new ResizeObserver(paint);
    resized.observe(target);
    // Without a blur the canvas has the screen's pixels, which moving the
    // window to another screen changes without resizing it.
    let scale: MediaQueryList | null = null;
    function watchScale() {
      scale?.removeEventListener("change", rescaled);
      scale = window.matchMedia(`(resolution: ${window.devicePixelRatio}dppx)`);
      scale.addEventListener("change", rescaled);
    }
    function rescaled() {
      paint();
      watchScale();
    }
    if (blur <= 0) watchScale();
    return () => {
      cancelled = true;
      resized.disconnect();
      scale?.removeEventListener("change", rescaled);
    };
  }, [image, blur, saturation]);

  return (
    <canvas
      ref={canvas}
      aria-hidden="true"
      className={className}
      // A color matrix only, cheap compared with a live blur.
      style={canFilter || saturation === 1 ? undefined : { filter: `saturate(${saturation})` }}
    />
  );
}

function draw(canvas: HTMLCanvasElement, image: HTMLImageElement, blur: number, saturation: number) {
  if (canvas.clientWidth === 0 || canvas.clientHeight === 0 || image.naturalWidth === 0) return;
  const size = stillBlurSize({ width: canvas.clientWidth, height: canvas.clientHeight }, blur, window.devicePixelRatio);
  const art = { width: image.naturalWidth, height: image.naturalHeight };
  canvas.width = size.width;
  canvas.height = size.height;
  const context = canvas.getContext("2d");
  if (!context) return;
  context.imageSmoothingQuality = "high";
  if (canFilter) {
    // The image reaches past the edges, where the blur would fade.
    const rect = coverRect(size, art, Math.ceil(size.radius * BACKGROUND_BLUR_BLEED));
    context.filter = `${size.radius > 0 ? `blur(${size.radius}px) ` : ""}saturate(${saturation})`;
    context.drawImage(image, rect.x, rect.y, rect.width, rect.height);
    return;
  }
  // Each box pass takes the radius off every edge, so the image starts that
  // much larger.
  const radius = Math.round(size.radius);
  const pad = radius * STILL_BLUR_BOX_PASSES;
  const padded = { width: size.width + pad * 2, height: size.height + pad * 2 };
  let blurred = shrunk(image, padded, coverRect(padded, art, 0));
  for (let pass = 0; radius > 0 && pass < STILL_BLUR_BOX_PASSES; pass++) {
    blurred = boxed(boxed(blurred, radius, true), radius, false);
  }
  context.drawImage(blurred, 0, 0);
}

function sized(width: number, height: number) {
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, width);
  canvas.height = Math.max(1, height);
  return canvas;
}

/**
 * The image drawn into a canvas of `size` at `rect`, halved step by step on
 * the way, so no detail flickers through a large step down.
 */
function shrunk(image: HTMLImageElement, size: { width: number; height: number }, rect: ReturnType<typeof coverRect>) {
  let source: HTMLImageElement | HTMLCanvasElement = image;
  let width = image.naturalWidth;
  let height = image.naturalHeight;
  while (width / 2 >= rect.width && height / 2 >= rect.height) {
    width = Math.round(width / 2);
    height = Math.round(height / 2);
    const step = sized(width, height);
    step.getContext("2d")?.drawImage(source, 0, 0, width, height);
    source = step;
  }
  const canvas = sized(size.width, size.height);
  canvas.getContext("2d")?.drawImage(source, rect.x, rect.y, rect.width, rect.height);
  return canvas;
}

/**
 * The mean of every pixel within `radius` along one axis, a canvas that much
 * smaller at both ends. The copies are laid over each other, the n-th at a
 * weight of 1/n, which leaves their mean. It reads no pixels, which a canvas
 * with art from the app's own scheme refuses.
 */
function boxed(source: HTMLCanvasElement, radius: number, across: boolean) {
  const canvas = sized(source.width - (across ? radius * 2 : 0), source.height - (across ? 0 : radius * 2));
  const context = canvas.getContext("2d");
  if (!context) return canvas;
  for (let step = 0; step <= radius * 2; step++) {
    context.globalAlpha = 1 / (step + 1);
    context.drawImage(source, across ? -step : 0, across ? 0 : -step);
  }
  return canvas;
}
