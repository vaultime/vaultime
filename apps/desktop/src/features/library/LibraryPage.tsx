// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useMemo, useState } from "react";
import { useSearchParams } from "react-router";
import type { Game } from "@/lib/types";
import { summarizeRecentPlay } from "@/lib/session-stats";
import { AddGameDialog } from "./components/AddGameDialog";
import { DeleteGameDialog } from "./components/DeleteGameDialog";
import { DiscoverGamesDialog } from "./components/DiscoverGamesDialog";
import { EditGameDialog } from "./components/EditGameDialog";
import { GameGrid } from "./components/GameGrid";
import { LibraryHero } from "./components/LibraryHero";
import { RecentStats } from "./components/RecentStats";
import { Shelf } from "./components/Shelf";
import { useLibrary } from "./library-context";

type LibraryDialog = "add" | "discover";

export function LibraryPage() {
  const { summaries, visible, sessions, active, loaded, error, refresh } = useLibrary();
  const [searchParams, setSearchParams] = useSearchParams();
  const [editing, setEditing] = useState<Game | null>(null);
  const [deleting, setDeleting] = useState<Game | null>(null);

  // The add and discover dialogs live in the URL, so the command palette can open them.
  const dialog: LibraryDialog | null =
    searchParams.get("add") === "1" ? "add" : searchParams.get("discover") === "1" ? "discover" : null;
  const setDialog = (next: LibraryDialog | null) =>
    setSearchParams(next ? { [next]: "1" } : {}, { replace: true });
  const openAdd = () => setDialog("add");
  const openDiscover = () => setDialog("discover");

  const playing = useMemo(() => new Set(active.map((session) => session.game_id)), [active]);
  const recent = useMemo(() => summarizeRecentPlay(sessions), [sessions]);

  const featured =
    visible.find((summary) => playing.has(summary.game.id)) ?? visible.find((summary) => summary.lastPlayedAt) ?? null;
  const shelf = visible.filter((summary) => summary.lastPlayedAt && summary !== featured);

  if (!loaded) return null;

  return (
    <div className="pb-16">
      {visible.length === 0 ? (
        <LibraryHero kind="welcome" onDiscover={openDiscover} onAdd={openAdd} />
      ) : featured ? (
        <LibraryHero
          kind="game"
          summary={featured}
          playing={playing.has(featured.game.id)}
          weekRuntimeMs={recent.runtimeByGame.get(featured.game.id) ?? 0}
        />
      ) : (
        <LibraryHero kind="unplayed" gameCount={visible.length} onDiscover={openDiscover} onAdd={openAdd} />
      )}

      {error && (
        <p role="alert" className="border-b border-rule px-8 py-3 text-sm text-amber xl:px-14">
          Could not load your library: {error}
        </p>
      )}

      {featured && <RecentStats recent={recent} summaries={summaries} />}

      {shelf.length > 0 && (
        <Shelf
          games={shelf}
          playing={playing}
          totalCount={visible.length}
          onShowAll={() => document.getElementById("all-games")?.scrollIntoView({ behavior: "smooth" })}
        />
      )}

      {visible.length > 0 && (
        <GameGrid
          summaries={visible}
          playing={playing}
          onDiscover={openDiscover}
          onAdd={openAdd}
          onEdit={setEditing}
          onDelete={setDeleting}
        />
      )}

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
