// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useLayoutEffect, useRef, useState, type KeyboardEvent, type PointerEvent, type RefObject } from "react";
import { Maximize2, Minimize2 } from "lucide-react";
import { Notice } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Slider } from "@/components/ui/slider";
import {
  COVER_ASPECT,
  COVER_WIDTH_PX,
  CROP_KEY_PAN_SHARE,
  CROP_SLIDER_STEPS,
  CROP_SOFT_SCALE,
  CROP_WHEEL_NOTCH_PX,
  CROP_ZOOM_STEP,
} from "@/lib/constants";
import {
  fillView,
  fitView,
  imagePlacement,
  initialView,
  panBy,
  sameView,
  toCrop,
  zoomAt,
  zoomBounds,
  type CropView,
  type FrameShare,
  type ImageSize,
} from "@/lib/crop";
import type { ArtworkSource, CropRect, GameAssetView } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";

/** An image to frame and what saving the frame does. */
export interface CropTarget {
  source: ArtworkSource;
  /** Names the image, like its file name. */
  label: string;
  save: (crop: CropRect) => Promise<GameAssetView[]>;
}

/** Frames an image as a cover: drag to move, wheel, slider or keys to zoom. */
export function CoverCropDialog({
  gameTitle,
  target,
  onClose,
  onSaved,
}: {
  gameTitle: string;
  target: CropTarget | null;
  onClose: () => void;
  onSaved: (assets: GameAssetView[]) => void;
}) {
  // The last image stays while the dialog animates out, and a dialog that is
  // saving does not close.
  const [shown, setShown] = useState(target);
  const [saving, setSaving] = useState(false);
  if (target && target !== shown) setShown(target);
  const close = () => {
    if (!saving) onClose();
  };
  return (
    <Dialog
      open={target !== null}
      onOpenChange={(open) => !open && close()}
      onOpenChangeComplete={(open) => !open && setShown(null)}
    >
      <DialogContent className="sm:max-w-[720px]">
        {shown && (
          <CropEditor
            key={shown.source.preview_data_url}
            gameTitle={gameTitle}
            target={shown}
            saving={saving}
            onSavingChange={setSaving}
            onClose={close}
            onSaved={onSaved}
          />
        )}
      </DialogContent>
    </Dialog>
  );
}

function CropEditor({
  gameTitle,
  target,
  saving,
  onSavingChange,
  onClose,
  onSaved,
}: {
  gameTitle: string;
  target: CropTarget;
  saving: boolean;
  onSavingChange: (saving: boolean) => void;
  onClose: () => void;
  onSaved: (assets: GameAssetView[]) => void;
}) {
  const { source } = target;
  const [size] = useState<ImageSize>(() => ({ width: source.width, height: source.height }));
  const [view, setView] = useState<CropView>(() => initialView(size, source.crop));
  const [error, setError] = useState<string | null>(null);
  const stageRef = useRef<HTMLDivElement>(null);
  const frameRef = useRef<HTMLDivElement>(null);
  const frameWidth = useWidth(frameRef);
  const drag = useRef<{ x: number; y: number } | null>(null);

  const fill = fillView(size);
  const fit = fitView(size);
  const placement = frameWidth ? imagePlacement(view, size, frameWidth) : null;
  const soft = COVER_WIDTH_PX / (toCrop(view, size).width * size.width) > CROP_SOFT_SCALE;

  // Wheel zoom keeps the point under the pointer. React wheel handlers are
  // passive, so the dialog would scroll along without a listener of our own.
  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;
    const onWheel = (event: WheelEvent) => {
      const frame = frameRef.current?.getBoundingClientRect();
      if (!frame) return;
      event.preventDefault();
      const anchor = { x: (event.clientX - frame.left) / frame.width, y: (event.clientY - frame.top) / frame.height };
      setView((current) => zoomAt(current, size, CROP_ZOOM_STEP ** (-event.deltaY / CROP_WHEEL_NOTCH_PX), anchor));
    };
    stage.addEventListener("wheel", onWheel, { passive: false });
    return () => stage.removeEventListener("wheel", onWheel);
  }, [size]);

  function startDrag(event: PointerEvent<HTMLDivElement>) {
    if (event.button !== 0) return;
    event.currentTarget.setPointerCapture(event.pointerId);
    drag.current = { x: event.clientX, y: event.clientY };
  }

  function moveDrag(event: PointerEvent<HTMLDivElement>) {
    const last = drag.current;
    if (!last || !frameWidth) return;
    drag.current = { x: event.clientX, y: event.clientY };
    const move = {
      x: (event.clientX - last.x) / frameWidth,
      y: ((event.clientY - last.y) * COVER_ASPECT) / frameWidth,
    };
    setView((current) => panBy(current, size, move));
  }

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const moves: Record<string, FrameShare> = {
      ArrowLeft: { x: -CROP_KEY_PAN_SHARE, y: 0 },
      ArrowRight: { x: CROP_KEY_PAN_SHARE, y: 0 },
      ArrowUp: { x: 0, y: -CROP_KEY_PAN_SHARE },
      ArrowDown: { x: 0, y: CROP_KEY_PAN_SHARE },
    };
    const zooms: Record<string, number> = { "+": CROP_ZOOM_STEP, "=": CROP_ZOOM_STEP, "-": 1 / CROP_ZOOM_STEP, _: 1 / CROP_ZOOM_STEP };
    if (event.key in moves) setView((current) => panBy(current, size, moves[event.key]));
    else if (event.key in zooms) setView((current) => zoomAt(current, size, zooms[event.key]));
    else return;
    event.preventDefault();
  }

  async function save() {
    onSavingChange(true);
    setError(null);
    try {
      const saved = await target.save(toCrop(view, size));
      onSavingChange(false);
      onSaved(saved);
      onClose();
    } catch (saveError) {
      setError(describeError(saveError));
      onSavingChange(false);
    }
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle>Frame the cover</DialogTitle>
        <DialogDescription>
          Drag the image to move it and zoom with the wheel or the slider. Zoom out to show a wide logo whole.
        </DialogDescription>
      </DialogHeader>

      <div className="grid gap-7 sm:grid-cols-[auto_minmax(0,1fr)] [@media(max-height:680px)]:gap-5">
        <div
          ref={stageRef}
          tabIndex={0}
          role="group"
          aria-label="Cover frame. Arrow keys move the image, plus and minus zoom."
          onPointerDown={startDrag}
          onPointerMove={moveDrag}
          onPointerUp={() => (drag.current = null)}
          onPointerCancel={() => (drag.current = null)}
          onKeyDown={onKeyDown}
          className="relative flex cursor-grab touch-none items-center justify-center overflow-hidden rounded-lg border border-rule bg-ink p-6 outline-none [@media(max-height:680px)]:p-4 select-none focus-visible:ring-2 focus-visible:ring-violet/60 active:cursor-grabbing"
        >
          <div ref={frameRef} className="relative w-[228px] [@media(max-height:680px)]:w-[176px]">
            {/* The image beyond the frame, faint, so the player sees what is left out. */}
            {placement && (
              <img
                src={source.preview_data_url}
                alt=""
                draggable={false}
                className="pointer-events-none absolute max-w-none opacity-25"
                style={placement}
              />
            )}
            <CoverComposition
              source={source}
              view={view}
              size={size}
              className="rounded-md ring-1 ring-hairline-strong"
            />
          </div>
        </div>

        <div className="flex min-w-0 flex-col gap-6 [@media(max-height:680px)]:gap-4">
          <section>
            <h3 className="label-caps">Image</h3>
            <p className="mt-2 flex items-baseline gap-3 text-sm">
              <span className="min-w-0 truncate text-soft" title={target.label}>
                {target.label}
              </span>
              <span className="shrink-0 font-mono text-xs text-faint">
                {source.width} × {source.height}
              </span>
            </p>
            {!source.from_original && (
              <p className="mt-1 text-[13px] text-faint">The original file is gone or has changed.</p>
            )}
          </section>

          <section>
            <h3 className="label-caps">Zoom</h3>
            <ZoomSlider view={view} size={size} onChange={setView} />
            <div className="mt-3 flex flex-wrap gap-2">
              <Button
                variant="outline"
                size="sm"
                aria-pressed={sameView(view, fill, size)}
                className="aria-pressed:border-faint aria-pressed:bg-raised"
                onClick={() => setView(fill)}
              >
                <Maximize2 />
                Fill
              </Button>
              <Button
                variant="outline"
                size="sm"
                aria-pressed={sameView(view, fit, size)}
                className="aria-pressed:border-faint aria-pressed:bg-raised"
                onClick={() => setView(fit)}
              >
                <Minimize2 />
                Fit whole image
              </Button>
            </div>
            {soft && (
              <p className="mt-2.5 text-[13px] text-faint">The image is small for this zoom, the cover may look soft.</p>
            )}
          </section>

          <section>
            <h3 className="label-caps">In the library</h3>
            <div className="mt-3 flex items-end gap-5">
              <CoverComposition
                source={source}
                view={view}
                size={size}
                className="w-20 rounded-md ring-1 ring-rule [@media(max-height:680px)]:w-14"
              />
              <div className="flex min-w-0 items-center gap-2.5 pb-1">
                {/* The rail shows the middle square of the cover. */}
                <div className="relative size-9 shrink-0 overflow-hidden rounded-md ring-1 ring-rule">
                  <CoverComposition
                    source={source}
                    view={view}
                    size={size}
                    className="absolute top-1/2 left-0 w-full -translate-y-1/2"
                  />
                </div>
                <span className="truncate text-sm text-soft">{gameTitle}</span>
              </div>
            </div>
          </section>
        </div>
      </div>

      {error && <Notice tone="warning">{error}</Notice>}

      <DialogFooter>
        <Button type="button" variant="ghost" disabled={saving} onClick={onClose}>
          Cancel
        </Button>
        <Button type="button" disabled={saving} onClick={() => void save()}>
          {saving ? "Saving" : "Use as cover"}
        </Button>
      </DialogFooter>
    </>
  );
}

/** The zoom on a log scale, so each step of the slider feels the same. */
function ZoomSlider({
  view,
  size,
  onChange,
}: {
  view: CropView;
  size: ImageSize;
  onChange: (update: (current: CropView) => CropView) => void;
}) {
  const { min, max } = zoomBounds(size);
  const range = Math.log(max / min);
  const position = (Math.log(view.zoom / min) / range) * CROP_SLIDER_STEPS;

  return (
    <Slider
      label="Zoom"
      value={position}
      min={0}
      max={CROP_SLIDER_STEPS}
      valueText={() =>
        sameView(view, fitView(size), size) ? "Whole image" : `${Math.round(view.zoom * 100)} percent of filling the cover`
      }
      onChange={(value) =>
        onChange((current) => zoomAt(current, size, (min * Math.exp((value / CROP_SLIDER_STEPS) * range)) / current.zoom))
      }
      className="mt-2"
    />
  );
}

/** The cover as it will be cut: the backdrop with the image placed on it. */
function CoverComposition({
  source,
  view,
  size,
  className,
}: {
  source: ArtworkSource;
  view: CropView;
  size: ImageSize;
  className?: string;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const width = useWidth(ref);
  const placement = width ? imagePlacement(view, size, width) : null;

  return (
    <div ref={ref} aria-hidden="true" className={cn("relative aspect-[3/4] overflow-hidden", className)}>
      <img src={source.backdrop_data_url} alt="" draggable={false} className="absolute inset-0 size-full" />
      {placement && (
        <img
          src={source.preview_data_url}
          alt=""
          draggable={false}
          className="pointer-events-none absolute max-w-none"
          style={placement}
        />
      )}
    </div>
  );
}

/**
 * The layout width of an element, kept current as it resizes. Read before
 * the first paint, and without transforms, so the dialog's opening zoom does
 * not skew it.
 */
function useWidth(ref: RefObject<HTMLElement | null>): number {
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const element = ref.current;
    if (!element) return;
    setWidth(element.offsetWidth);
    const observer = new ResizeObserver(() => setWidth(element.offsetWidth));
    observer.observe(element);
    return () => observer.disconnect();
  }, [ref]);
  return width;
}
