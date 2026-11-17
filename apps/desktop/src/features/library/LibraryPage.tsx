// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import { Navigate, useSearchParams } from "react-router";
import { Button } from "@/components/ui/button";
import { timeOnLauncher } from "@/lib/launchers";
import type { Game } from "@/lib/types";
import { AddGameDialog } from "./components/AddGameDialog";
import { DeleteGameDialog } from "./components/DeleteGameDialog";
import { DiscoverGamesDialog } from "./components/DiscoverGamesDialog";
import { EditGameDialog } from "./components/EditGameDialog";
import { GameGrid } from "./components/GameGrid";
import { useLibrary, usePlaying } from "./library-context";

type LibraryDialog = "add" | "discover";

/** All games, with the ways to add more. */
export function LibraryPage() {
  const { summaries, visible, loaded, error, refresh } = useLibrary();
  const playing = usePlaying();
  const [searchParams, setSearchParams] = useSearchParams();
  const [editing, setEditing] = useState<Game | null>(null);
  const [deleting, setDeleting] = useState<Game | null>(null);

  // The add and discover dialogs live in the URL, so the command palette can open them.
  const dialog: LibraryDialog | null =
    searchParams.get("add") === "1" ? "add" : searchParams.get("discover") === "1" ? "discover" : null;
  const setDialog = (next: LibraryDialog | null) =>
    setSearchParams(
      (params) => {
        params.delete("add");
        params.delete("discover");
        if (next) params.set(next, "1");
        return params;
      },
      { replace: true },
    );
  const favorites = searchParams.get("favorites") === "1";
  const launcher = favorites ? null : searchParams.get("launcher");
  // On a launcher's view a game shows and sorts by its time there.
  const shown = favorites
    ? visible.filter((summary) => summary.favorite)
    : launcher
      ? visible
          .filter((summary) => summary.launchers.includes(launcher))
          .map((summary) => ({ ...summary, totalMs: timeOnLauncher(summary, launcher) }))
      : visible;
  const openAdd = () => setDialog("add");
  const openDiscover = () => setDialog("discover");

  if (!loaded) return null;
  // A launcher whose last game went away.
  if (launcher && shown.length === 0) return <Navigate to="/library" replace />;

  // Nothing came in, which says nothing about whether there are games.
  if (error && summaries.length === 0) {
    return (
      <div className="px-8 pt-12 xl:px-14">
        <h1 className="font-display text-[28px]">Your library did not load</h1>
        <p role="alert" className="mt-3 max-w-[560px] text-amber">
          {error}
        </p>
        <Button variant="outline" className="mt-6" onClick={() => void refresh()}>
          Try again
        </Button>
      </div>
    );
  }

  return (
    <div className="pb-16">
      {error && (
        <p role="alert" className="border-b border-rule px-8 py-3 text-sm text-amber xl:px-14">
          Could not load your library: {error}
        </p>
      )}

      <GameGrid
        summaries={shown}
        launcher={launcher}
        favorites={favorites}
        playing={playing}
        onDiscover={openDiscover}
        onAdd={openAdd}
        onEdit={setEditing}
        onDelete={setDeleting}
      />

      <AddGameDialog
        open={dialog === "add"}
        onOpenChange={(open) => setDialog(open ? "add" : null)}
        onAdded={refresh}
      />
      <DiscoverGamesDialog
        open={dialog === "discover"}
        onOpenChange={(open) => setDialog(open ? "discover" : null)}
        onImported={refresh}
      />
      <EditGameDialog game={editing} onClose={() => setEditing(null)} onSaved={refresh} />
      <DeleteGameDialog game={deleting} onClose={() => setDeleting(null)} onDeleted={refresh} />
    </div>
  );
}
