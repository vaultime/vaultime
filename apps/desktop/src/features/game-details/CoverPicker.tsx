// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useRef, useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { Crop, ImagePlus, Loader2, RefreshCw, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { CoverCropDialog, type CropTarget } from "@/features/game-details/CoverCropDialog";
import { ARTWORK_DIALOG_EXTENSIONS, PLAYER_ARTWORK_SOURCE } from "@/lib/constants";
import * as api from "@/lib/tauri";
import type { GameAssetView } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";
import { AsideSection } from "./Aside";

/** The last part of a path, on Windows or Linux. */
function fileName(path: string): string {
  return path.split(/[\\/]/).pop() || path;
}

/** The game's cover images: the one in use, the ones to pick from, and adding or framing one. */
export function CoverPicker({
  gameId,
  gameTitle,
  assets,
  onChanged,
  scannable = true,
}: {
  gameId: string;
  gameTitle: string;
  assets: GameAssetView[];
  onChanged: (assets: GameAssetView[]) => void;
  /** False for a game of another PC, whose folder is not on this PC. */
  scannable?: boolean;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [cropTarget, setCropTarget] = useState<CropTarget | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const withPreview = assets.filter((asset) => asset.preview_data_url);
  const inUse = withPreview.find((asset) => asset.is_preferred);
  const added = withPreview.filter((asset) => asset.source === PLAYER_ARTWORK_SOURCE);
  const found = withPreview.filter((asset) => asset.source !== PLAYER_ARTWORK_SOURCE);
  const toDelete = withPreview.find((asset) => asset.id === deleting);

  async function run(task: () => Promise<GameAssetView[] | null>, failure: string) {
    try {
      setBusy(true);
      setError(null);
      const next = await task();
      if (next) onChanged(next);
    } catch (taskError) {
      setError(`${failure}: ${describeError(taskError)}`);
    } finally {
      setBusy(false);
    }
  }

  const rescan = () => run(() => api.scanGameAssets(gameId), "The scan failed");
  const choose = (assetId: string) =>
    run(async () => {
      await api.setPreferredGameAsset(gameId, assetId);
      return api.listGameAssets(gameId);
    }, "Could not use that image");
  const addImage = () =>
    run(async () => {
      const selected = await openFileDialog({
        multiple: false,
        directory: false,
        title: "Choose a cover image",
        filters: [{ name: "Images", extensions: ARTWORK_DIALOG_EXTENSIONS }],
      });
      if (!selected) return null;
      const source = await api.openArtworkFile(selected);
      setCropTarget({ source, label: fileName(selected), save: (crop) => api.importGameAsset(gameId, selected, crop) });
      return null;
    }, "Could not open the image");
  const adjust = (asset: GameAssetView) =>
    run(async () => {
      const source = await api.openGameAssetSource(gameId, asset.id);
      setCropTarget({
        source,
        label: source.from_original ? fileName(asset.file_path) : "The cover as it is now",
        save: (crop) => api.cropGameAsset(gameId, asset.id, crop),
      });
      return null;
    }, "Could not open the cover");
  const remove = (asset: GameAssetView) =>
    run(async () => {
      const next = await api.deleteGameAsset(gameId, asset.id);
      setDeleting(null);
      return next;
    }, "Could not delete the image");

  return (
    <AsideSection title="Cover">
      {withPreview.length === 0 ? (
        <p className="text-[13px] text-faint">
          {scannable ? "No artwork yet. Scan the game folder or add an image." : "No artwork yet. Add an image."}
        </p>
      ) : (
        <div className="flex flex-col gap-4">
          <CoverGroup
            title="Added by you"
            empty="None yet. Add an image to frame it yourself."
            noun="your image"
            assets={added}
            busy={busy}
            deleting={deleting}
            onChoose={choose}
            onDelete={setDeleting}
          />
          <CoverGroup
            title={scannable ? "Found on this PC" : "Came with its sessions"}
            empty={scannable ? "Nothing found. Scan the folder to look again." : "None came along."}
            noun="found image"
            assets={found}
            busy={busy}
            deleting={deleting}
            onChoose={choose}
            onDelete={setDeleting}
          />
        </div>
      )}
      {toDelete && (
        <DeleteCoverConfirm
          asset={toDelete}
          busy={busy}
          onDelete={() => remove(toDelete)}
          onKeep={() => setDeleting(null)}
        />
      )}
      <div className="mt-3 flex flex-wrap gap-2">
        {scannable && (
          <Button variant="outline" size="sm" onClick={rescan} disabled={busy}>
            {busy ? <Loader2 className="size-3.5 animate-spin" /> : <RefreshCw className="size-3.5" />}
            Scan folder
          </Button>
        )}
        <Button variant="outline" size="sm" onClick={addImage} disabled={busy}>
          <ImagePlus className="size-3.5" />
          Add image
        </Button>
        {inUse && (
          <Button variant="outline" size="sm" onClick={() => adjust(inUse)} disabled={busy}>
            <Crop className="size-3.5" />
            Adjust
          </Button>
        )}
      </div>
      {error && (
        <p role="alert" className="mt-2.5 text-[13px] text-amber">
          {error}
        </p>
      )}
      <CoverCropDialog
        gameTitle={gameTitle}
        target={cropTarget}
        onClose={() => setCropTarget(null)}
        onSaved={onChanged}
      />
    </AsideSection>
  );
}

/** One group of cover images, each with a button to use it and one to delete it. */
function CoverGroup({
  title,
  empty,
  noun,
  assets,
  busy,
  deleting,
  onChoose,
  onDelete,
}: {
  title: string;
  empty: string;
  /** How the buttons name an image, like "your image 2". */
  noun: string;
  assets: GameAssetView[];
  busy: boolean;
  deleting: string | null;
  onChoose: (assetId: string) => void;
  onDelete: (assetId: string) => void;
}) {
  return (
    <div>
      <div className="mb-2 flex items-baseline gap-2 text-[13px]">
        <span className="text-soft">{title}</span>
        <span className="text-faint tabular-nums">{assets.length}</span>
      </div>
      {assets.length === 0 ? (
        <p className="text-[13px] text-faint">{empty}</p>
      ) : (
        <div className="flex flex-wrap gap-2.5">
          {assets.map((asset, index) => {
            const name = `${noun} ${index + 1}`;
            const marked = asset.id === deleting;
            return (
              <div key={asset.id} className="group relative">
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onChoose(asset.id)}
                  aria-pressed={asset.is_preferred}
                  aria-label={`Use ${name}${asset.is_preferred ? ", in use" : ""}`}
                  className={cn(
                    "block h-24 w-[72px] overflow-hidden rounded-[5px] border transition focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:outline-none",
                    asset.is_preferred ? "border-2 border-text" : "border-rule opacity-70 hover:opacity-100",
                    marked && "border-destructive opacity-100",
                  )}
                >
                  <img src={asset.preview_data_url ?? undefined} alt="" className="size-full object-cover" />
                </button>
                <button
                  type="button"
                  disabled={busy}
                  onClick={() => onDelete(asset.id)}
                  aria-label={`Delete ${name}`}
                  className={cn(
                    "absolute -top-1.5 -right-1.5 flex size-5 items-center justify-center rounded-full border border-hairline-strong bg-surface text-soft opacity-0 transition group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-violet/70 focus-visible:outline-none",
                    marked && "border-destructive text-destructive opacity-100",
                  )}
                >
                  <X className="size-3" />
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** Asks before an image goes, in the section rather than a dialog. */
function DeleteCoverConfirm({
  asset,
  busy,
  onDelete,
  onKeep,
}: {
  asset: GameAssetView;
  busy: boolean;
  onDelete: () => void;
  onKeep: () => void;
}) {
  const keep = useRef<HTMLButtonElement>(null);
  // The safe answer has the focus, so Enter keeps the image.
  useEffect(() => keep.current?.focus(), [asset.id]);
  const found = asset.source !== PLAYER_ARTWORK_SOURCE;

  return (
    <div
      role="group"
      aria-label="Delete this image?"
      className="mt-3 border-t border-rule pt-3"
      onKeyDown={(event) => {
        if (event.key === "Escape") onKeep();
      }}
    >
      <p className="text-[13px] leading-relaxed text-soft">
        Delete this image from Vaultime? The file it came from stays where it is.
        {asset.is_preferred && " The next image becomes the cover."}
        {found && " Scan folder finds it again."}
      </p>
      <div className="mt-2.5 flex gap-2">
        <Button variant="destructive" size="sm" onClick={onDelete} disabled={busy}>
          <Trash2 className="size-3.5" />
          Delete
        </Button>
        <Button ref={keep} variant="ghost" size="sm" onClick={onKeep} disabled={busy}>
          Keep it
        </Button>
      </div>
    </div>
  );
}

