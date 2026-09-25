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
}

/** Infers a game title from a file or folder path. */
function inferTitle(path: string): string {
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const base = normalized.split("/").pop() ?? "";
  // Strip common executable extensions
  return base.replace(/\.(exe|app|sh|bat|cmd|lnk)$/i, "").replace(/[_-]/g, " ");
}

export function AddGameDialog({ onAdded }: AddGameDialogProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [title, setTitle] = useState("");
  const [executablePath, setExecutablePath] = useState("");
  const [installFolder, setInstallFolder] = useState("");
  const [saving, setSaving] = useState(false);

  function reset() {
    setTitle("");
    setExecutablePath("");
    setInstallFolder("");
    setSaving(false);
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
      const path = typeof selected === "string" ? selected : selected;
      setExecutablePath(path);
      if (!title) {
        setTitle(inferTitle(path));
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
      const path = typeof selected === "string" ? selected : selected;
      setInstallFolder(path);
      if (!title) {
        setTitle(inferTitle(path));
      }
    }
  }

  async function handleSave() {
    if (!title.trim()) return;
    setSaving(true);
    try {
      await api.createGame({
        title: title.trim(),
        executable_path: executablePath || null,
        install_folder: installFolder || null,
      });
      setIsOpen(false);
      reset();
      onAdded();
    } catch (e) {
      console.error("Failed to add game:", e);
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog
      open={isOpen}
      onOpenChange={(open) => {
        setIsOpen(open);
        if (!open) reset();
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
