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
import * as api from "@/lib/tauri";
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
      setError(`Could not delete the game: ${String(e)}`);
    } finally {
      setDeleting(false);
    }
  }

  return (
    <Dialog open={!!game} onOpenChange={(open) => !open && close()}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>Delete Game</DialogTitle>
          <DialogDescription>
            Are you sure you want to delete{" "}
            <strong className="text-foreground">{game?.title}</strong>? This will
            remove all session history for this game.
          </DialogDescription>
        </DialogHeader>
        {error && (
          <div className="rounded-2xl border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
            {error}
          </div>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={close}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            onClick={handleDelete}
            disabled={deleting}
          >
            {deleting ? "Deleting..." : "Delete"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
