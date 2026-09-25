// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Folder, FileIcon } from "lucide-react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import * as api from "@/lib/tauri";
import type { Game } from "@/lib/types";

interface EditGameDialogProps {
  game: Game | null;
  onClose: () => void;
  onSaved: () => void;
}

export function EditGameDialog({ game, onClose, onSaved }: EditGameDialogProps) {
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

  async function pickExecutable() {
    const selected = await open({
      multiple: false,
      directory: false,
      title: "Select game executable",
      filters: [
        {
          name: "Executables",
          extensions: ["exe", "app", "sh", "bat", "cmd", ""],
        },
        { name: "All files", extensions: ["*"] },
      ],
    });
    if (selected) {
      setExecutablePath(selected);
    }
  }

  async function pickFolder() {
    const selected = await open({
      multiple: false,
      directory: true,
      title: "Select game install folder",
    });
    if (selected) {
      setInstallFolder(selected);
    }
  }

  async function handleSave() {
    if (!game || !title.trim()) return;
    setSaving(true);
    setError(null);
    try {
      const pathsChanged =
        executablePath !== (game.executable_path ?? "") ||
        installFolder !== (game.install_folder ?? "");

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
    } catch (e) {
      setError(`Could not save the game: ${String(e)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={!!game} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Edit Game</DialogTitle>
          <DialogDescription>
            Update game details and paths.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label htmlFor="edit-title">Title</Label>
            <Input
              id="edit-title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </div>

          <div className="space-y-2">
            <Label>Executable</Label>
            <div className="flex gap-2">
              <Input
                readOnly
                placeholder="No executable selected"
                value={executablePath}
                className="flex-1"
              />
              <Button variant="outline" size="icon" onClick={pickExecutable}>
                <FileIcon className="h-4 w-4" />
              </Button>
            </div>
          </div>

          <div className="space-y-2">
            <Label>Install Folder</Label>
            <div className="flex gap-2">
              <Input
                readOnly
                placeholder="No folder selected"
                value={installFolder}
                className="flex-1"
              />
              <Button variant="outline" size="icon" onClick={pickFolder}>
                <Folder className="h-4 w-4" />
              </Button>
            </div>
          </div>

          {error && (
            <div className="rounded-2xl border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
              {error}
            </div>
          )}
        </div>

        <DialogFooter>
          <Button variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!title.trim() || saving}>
            {saving ? "Saving..." : "Save"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
