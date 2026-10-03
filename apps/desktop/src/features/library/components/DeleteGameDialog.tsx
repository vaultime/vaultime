// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Notice } from "@/components/layout/Page";
import * as api from "@/lib/tauri";
import { describeError } from "@/lib/utils";
import type { Game } from "@/lib/types";

interface DeleteGameDialogProps {
  game: Game | null;
  onClose: () => void;
  onDeleted: () => void;
}

export function DeleteGameDialog({
  game,
  onClose,
  onDeleted,
}: DeleteGameDialogProps) {
  const [deleting, setDeleting] = useState(false);
  const [ignoreProgram, setIgnoreProgram] = useState(false);
  const [error, setError] = useState<string | null>(null);

  function close() {
    setError(null);
    setIgnoreProgram(false);
    onClose();
  }

  async function handleDelete() {
    if (!game) return;
    setDeleting(true);
    setError(null);
    try {
      await api.deleteGame(game.id);
      if (ignoreProgram && game.executable_path) await api.ignoreProgram(game.executable_path, game.title);
      onDeleted();
      close();
    } catch (e) {
      setError(`Could not delete the game: ${describeError(e)}`);
    } finally {
      setDeleting(false);
    }
  }

  return (
    <Dialog open={!!game} onOpenChange={(open) => !open && close()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>Delete this game?</DialogTitle>
          <DialogDescription>
            <strong className="font-medium text-text">{game?.title}</strong> and all of its sessions are
            removed from this PC. Backups you made earlier keep them.
          </DialogDescription>
        </DialogHeader>
        {game?.executable_path && (
          <label className="flex cursor-pointer items-start gap-2.5 text-sm text-soft">
            <input
              type="checkbox"
              checked={ignoreProgram}
              onChange={(event) => setIgnoreProgram(event.target.checked)}
              className="mt-0.5 accent-violet"
            />
            <span>
              It is not a game. Never suggest its program again and never track it.
            </span>
          </label>
        )}
        {error && <Notice tone="warning">{error}</Notice>}
        <DialogFooter>
          <Button variant="ghost" onClick={close}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            onClick={handleDelete}
            disabled={deleting}
          >
            {deleting ? "Deleting" : "Delete game"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
