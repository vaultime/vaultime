// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { COVER_ASPECT, CROP_FIT_MIN_ASPECT, CROP_MAX_ZOOM_OF_FILL, CROP_MIN_ZOOM_OF_FIT } from "@/lib/constants";
import type { CropRect } from "@/lib/types";

export interface ImageSize {
  width: number;
  height: number;
}

/**
 * Where the cover frame sits on an image. A zoom of 1 fills the frame, below
 * 1 the image shrinks inside it. The center is in image pixels.
 */
export interface CropView {
  zoom: number;
  centerX: number;
  centerY: number;
}

/** A point or distance as shares of the frame width and height. */
export interface FrameShare {
  x: number;
  y: number;
}

/** Frame width in image pixels at zoom 1, where the image just fills it. */
function fillWidth({ width, height }: ImageSize): number {
  return Math.min(width, height * COVER_ASPECT);
}

/** Frame width in image pixels where the whole image just fits. */
function fitWidth({ width, height }: ImageSize): number {
  return Math.max(width, height * COVER_ASPECT);
}

/** The zoom that fits the whole image into the frame. */
export function fitZoom(size: ImageSize): number {
  return fillWidth(size) / fitWidth(size);
}

/** How far the frame may zoom out, past fitting for a margin around a logo, and in. */
export function zoomBounds(size: ImageSize): { min: number; max: number } {
  return { min: fitZoom(size) * CROP_MIN_ZOOM_OF_FIT, max: CROP_MAX_ZOOM_OF_FILL };
}

function frameSize(view: CropView, size: ImageSize) {
  const width = fillWidth(size) / view.zoom;
  return { width, height: width / COVER_ASPECT };
}

/**
 * Keeps the zoom in bounds and the image in place: while the frame is inside
 * the image along an axis it may not leave it, and while the image is
 * inside the frame the image may not leave the frame.
 */
export function clampView(view: CropView, size: ImageSize): CropView {
  const { min, max } = zoomBounds(size);
  const zoom = Math.min(max, Math.max(min, view.zoom));
  const frame = frameSize({ ...view, zoom }, size);
  const clampAxis = (center: number, extent: number, frameExtent: number) => {
    const half = frameExtent / 2;
    const low = Math.min(half, extent - half);
    const high = Math.max(half, extent - half);
    return Math.min(high, Math.max(low, center));
  };
  return {
    zoom,
    centerX: clampAxis(view.centerX, size.width, frame.width),
    centerY: clampAxis(view.centerY, size.height, frame.height),
  };
}

/** The whole image in the middle of the frame. */
export function fitView(size: ImageSize): CropView {
  return { zoom: fitZoom(size), centerX: size.width / 2, centerY: size.height / 2 };
}

/** The middle of the image filling the frame. */
export function fillView(size: ImageSize): CropView {
  return { zoom: 1, centerX: size.width / 2, centerY: size.height / 2 };
}

/**
 * Where the dialog opens: the crop in use, or for a new image the whole of a
 * wide image and the middle of a tall one.
 */
export function initialView(size: ImageSize, crop: CropRect | null): CropView {
  if (crop) return viewFromCrop(crop, size);
  return size.width / size.height >= CROP_FIT_MIN_ASPECT ? fitView(size) : fillView(size);
}

/** The crop the core cuts, in shares of the image size. */
export function toCrop(view: CropView, size: ImageSize): CropRect {
  const frame = frameSize(view, size);
  return {
    x: (view.centerX - frame.width / 2) / size.width,
    y: (view.centerY - frame.height / 2) / size.height,
    width: frame.width / size.width,
    height: frame.height / size.height,
  };
}

export function viewFromCrop(crop: CropRect, size: ImageSize): CropView {
  const frameWidth = crop.width * size.width;
  return clampView(
    {
      zoom: fillWidth(size) / frameWidth,
      centerX: (crop.x + crop.width / 2) * size.width,
      centerY: (crop.y + crop.height / 2) * size.height,
    },
    size,
  );
}

/** Moves the image by shares of the frame, as when it is dragged. */
export function panBy(view: CropView, size: ImageSize, move: FrameShare): CropView {
  const frame = frameSize(view, size);
  return clampView(
    { ...view, centerX: view.centerX - move.x * frame.width, centerY: view.centerY - move.y * frame.height },
    size,
  );
}

/** Zooms by a factor and keeps the image point under `anchor` where it is. */
export function zoomAt(
  view: CropView,
  size: ImageSize,
  factor: number,
  anchor: FrameShare = { x: 0.5, y: 0.5 },
): CropView {
  const before = frameSize(view, size);
  const { min, max } = zoomBounds(size);
  const zoom = Math.min(max, Math.max(min, view.zoom * factor));
  const after = frameSize({ ...view, zoom }, size);
  const pointX = view.centerX - before.width / 2 + anchor.x * before.width;
  const pointY = view.centerY - before.height / 2 + anchor.y * before.height;
  return clampView(
    {
      zoom,
      centerX: pointX - anchor.x * after.width + after.width / 2,
      centerY: pointY - anchor.y * after.height + after.height / 2,
    },
    size,
  );
}

/** Where the image lands in a frame drawn `frameWidth` pixels wide, in pixels. */
export function imagePlacement(view: CropView, size: ImageSize, frameWidth: number) {
  const frame = frameSize(view, size);
  const scale = frameWidth / frame.width;
  return {
    left: (frame.width / 2 - view.centerX) * scale,
    top: (frame.height / 2 - view.centerY) * scale,
    width: size.width * scale,
    height: size.height * scale,
  };
}

/** Whether two views show the same crop, give or take rounding. */
export function sameView(a: CropView, b: CropView, size: ImageSize): boolean {
  const tolerance = 1e-6;
  const first = toCrop(a, size);
  const second = toCrop(b, size);
  return (["x", "y", "width", "height"] as const).every((key) => Math.abs(first[key] - second[key]) < tolerance);
}
