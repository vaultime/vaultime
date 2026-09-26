// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useState } from "react";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
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

interface AddGameDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onAdded: () => void;
}

function inferTitle(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const base = normalized.split("/").pop() ?? "";
  return base.replace(/\.(exe|app|sh|bat|cmd|lnk)$/i, "").replace(/[_-]/g, " ");
}

export function AddGameDialog({ open, onOpenChange, onAdded }: AddGameDialogProps) {
  const [title, setTitle] = useState("");
  const [executablePath, setExecutablePath] = useState("");
  const [installFolder, setInstallFolder] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function reset() {
    setTitle("");
    setExecutablePath("");
    setInstallFolder("");
    setSaving(false);
    setError(null);
  }

  // Start with an empty form every time the dialog opens.
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) reset();
  }

  async function pickExecutable() {
    const selected = await openFileDialog({
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
      if (!title) {
        setTitle(inferTitle(selected));
      }
    }
  }

  async function pickFolder() {
    const selected = await openFileDialog({
      multiple: false,
      directory: true,
      title: "Select game install folder",
    });
    if (selected) {
      setInstallFolder(selected);
      if (!title) {
        setTitle(inferTitle(selected));
      }
    }
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
    } catch (e) {
      setError(`Could not add the game: ${String(e)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add a game</DialogTitle>
          <DialogDescription>
            Select a game executable or install folder to start tracking.
          </DialogDescription>
        </DialogHeader>

        <div className="space-y-4 py-2">
          <div className="space-y-2">
            <Label htmlFor="title">Title</Label>
            <Input
              id="title"
              placeholder="Game title"
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
            <Label>Install folder</Label>
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
          <Button
            variant="ghost"
            onClick={() => onOpenChange(false)}
          >
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!title.trim() || saving}>
            {saving ? "Adding..." : "Add game"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
