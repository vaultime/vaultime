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
  DialogTrigger,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import * as api from "@/lib/tauri";

interface AddGameDialogProps {
  onAdded: () => void;
  autoOpen?: boolean;
  onDismiss?: () => void;
}

function inferTitle(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const base = normalized.split("/").pop() ?? "";
  return base.replace(/\.(exe|app|sh|bat|cmd|lnk)$/i, "").replace(/[_-]/g, " ");
}

export function AddGameDialog({ onAdded, autoOpen, onDismiss }: AddGameDialogProps) {
  const [isOpen, setIsOpen] = useState(false);
  // Opened from the command palette through the URL.
  const [autoOpened, setAutoOpened] = useState(false);
  if (autoOpen && !autoOpened) {
    setAutoOpened(true);
    setIsOpen(true);
  } else if (!autoOpen && autoOpened) {
    setAutoOpened(false);
  }
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
      if (!title) {
        setTitle(inferTitle(selected));
      }
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
      setIsOpen(false);
      reset();
      onAdded();
    } catch (e) {
      setError(`Could not add the game: ${String(e)}`);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        setIsOpen(open);
        if (!open) {
          reset();
          onDismiss?.();
        }
      }}
    >
      <DialogTrigger render={<Button />}>
        Add Game
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Add Game</DialogTitle>
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
          <Button
            variant="ghost"
            onClick={() => {
              setIsOpen(false);
              reset();
            }}
          >
            Cancel
          </Button>
          <Button onClick={handleSave} disabled={!title.trim() || saving}>
            {saving ? "Adding..." : "Add Game"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
