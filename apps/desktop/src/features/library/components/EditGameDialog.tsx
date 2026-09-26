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
import type { Game } from "@/lib/types";
import { describeError } from "@/lib/utils";
import { GameFields } from "./GameFields";

export function EditGameDialog({
  game,
  onClose,
  onSaved,
}: {
  game: Game | null;
  onClose: () => void;
  onSaved: () => void;
}) {
  const [title, setTitle] = useState("");
  const [executablePath, setExecutablePath] = useState("");
  const [installFolder, setInstallFolder] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [formGame, setFormGame] = useState<Game | null>(null);

  // Reset the form during render whenever the dialog opens for a game.
  if (game !== formGame) {
    setFormGame(game);
    if (game) {
      setTitle(game.title);
      setExecutablePath(game.executable_path ?? "");
      setInstallFolder(game.install_folder ?? "");
      setError(null);
    }
  }

  async function handleSave() {
    if (!game || !title.trim()) return;
    setSaving(true);
    setError(null);
    try {
      const pathsChanged =
        executablePath !== (game.executable_path ?? "") || installFolder !== (game.install_folder ?? "");
      await api.updateGame(game.id, {
        title: title.trim(),
        executable_path: executablePath || null,
        install_folder: installFolder || null,
      });
      if (pathsChanged && (executablePath || installFolder)) {
        await api.scanGameAssets(game.id).catch(() => {});
      }
      onSaved();
      onClose();
    } catch (saveError) {
      setError(`Could not save the game: ${describeError(saveError)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={!!game} onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Edit game</DialogTitle>
          <DialogDescription>A new program or folder is scanned for cover art again.</DialogDescription>
        </DialogHeader>

        <GameFields
          id="edit-game"
          title={title}
          onTitleChange={setTitle}
          executablePath={executablePath}
          onExecutableChange={setExecutablePath}
          installFolder={installFolder}
          onInstallFolderChange={setInstallFolder}
        />
        {error && <Notice tone="warning">{error}</Notice>}

        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!title.trim() || saving}>
            {saving ? "Saving" : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
