// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useState } from "react";
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
import * as api from "@/lib/tauri";
import { describeError } from "@/lib/utils";
import { GameFields } from "./GameFields";

/** "C:\Games\Hollow_Knight\hollow_knight.exe" gives "hollow knight". */
function inferTitle(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const base = normalized.split("/").pop() ?? "";
  return base.replace(/\.(exe|app|sh|bat|cmd|lnk)$/i, "").replace(/[_-]/g, " ");
}

export function AddGameDialog({
  open,
  onOpenChange,
  onAdded,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdded: () => void;
}) {
  const [title, setTitle] = useState("");
  const [executablePath, setExecutablePath] = useState("");
  const [installFolder, setInstallFolder] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Start with an empty form every time the dialog opens.
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) {
      setTitle("");
      setExecutablePath("");
      setInstallFolder("");
      setSaving(false);
      setError(null);
    }
  }

  function choosePath(setPath: (path: string) => void) {
    return (path: string) => {
      setPath(path);
      if (!title) setTitle(inferTitle(path));
    };
  }

  async function handleSave() {
    if (!title.trim()) return;
    setSaving(true);
    setError(null);
    try {
      const game = await api.createGame({
        title: title.trim(),
        executable_path: executablePath || null,
        install_folder: installFolder || null,
      });
      if (executablePath || installFolder) {
        await api.scanGameAssets(game.id).catch(() => {});
      }
      onOpenChange(false);
      onAdded();
    } catch (saveError) {
      setError(`Could not add the game: ${describeError(saveError)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Add a game</DialogTitle>
          <DialogDescription>Choose its program, and its install folder for cover art.</DialogDescription>
        </DialogHeader>

        <GameFields
          id="add-game"
          title={title}
          onTitleChange={setTitle}
          executablePath={executablePath}
          onExecutableChange={choosePath(setExecutablePath)}
          installFolder={installFolder}
          onInstallFolderChange={choosePath(setInstallFolder)}
        />
        {error && <Notice tone="warning">{error}</Notice>}

        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!title.trim() || saving}>
            {saving ? "Adding" : "Add game"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
