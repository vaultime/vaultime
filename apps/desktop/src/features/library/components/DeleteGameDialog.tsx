// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

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
  const [error, setError] = useState<string | null>(null);

  function close() {
    setError(null);
    onClose();
  }

  async function handleDelete() {
    if (!game) return;
    setDeleting(true);
    setError(null);
    try {
      await api.deleteGame(game.id);
      onDeleted();
      onClose();
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
