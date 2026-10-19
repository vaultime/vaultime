// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { useSearchParams } from "react-router";
import { Button } from "@/components/ui/button";
import { scrollMotion } from "@/lib/anchors";
import { LIVE_TOTALS_REFRESH_MS, RECENT_DAYS } from "@/lib/constants";
import * as api from "@/lib/tauri";
import type { Game, RecentPlay } from "@/lib/types";
import { useRefreshWhile } from "@/lib/use-refresh-while";
import { AddGameDialog } from "./components/AddGameDialog";
import { DeleteGameDialog } from "./components/DeleteGameDialog";
import { DiscoverGamesDialog } from "./components/DiscoverGamesDialog";
import { EditGameDialog } from "./components/EditGameDialog";
import { GameGrid } from "./components/GameGrid";
import { LibraryHero } from "./components/LibraryHero";
import { RecentStats } from "./components/RecentStats";
import { Shelf } from "./components/Shelf";
import { useLibrary, usePlaying } from "./library-context";

type LibraryDialog = "add" | "discover";

/** Shown when the core cannot sum up the last days, so the page still opens. */
const NO_RECENT_PLAY: RecentPlay = {
  played_ms: 0,
  runtime_ms: 0,
  active_ms: 0,
  sessions_count: 0,
  days_count: 0,
  games: [],
  longest_session_id: null,
};

export function LibraryPage() {
  const { summaries, visible, sessions, today, loaded, error, refresh } = useLibrary();
  const playing = usePlaying();
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

  // Read again whenever a session starts or ends, on a new day, and while a game runs.
  const [recent, setRecent] = useState<RecentPlay | null>(null);
  const refreshed = useRefreshWhile(playing.size > 0, LIVE_TOTALS_REFRESH_MS);
  useEffect(() => {
    let cancelled = false;
    api
      .getRecentPlay(RECENT_DAYS)
      .then((next) => {
        if (!cancelled) setRecent(next);
      })
      .catch(() => {
        if (!cancelled) setRecent((current) => current ?? NO_RECENT_PLAY);
      });
    return () => {
      cancelled = true;
    };
  }, [sessions, today, refreshed]);

  const featured =
    visible.find((summary) => playing.has(summary.game.id)) ?? visible.find((summary) => summary.lastPlayedAt) ?? null;
  const shelf = visible.filter((summary) => summary.lastPlayedAt && summary !== featured);

  // Waits for the last days too, so the hero's sentence does not change as it shows.
  if (!loaded || (featured && !recent)) return null;

  // Nothing came in, which says nothing about whether there are games.
  if (error && summaries.length === 0) {
    return (
      <div className="px-8 pt-12 xl:px-14">
        <h1 className="font-display text-4xl">Your library did not load</h1>
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
      {visible.length === 0 ? (
        <LibraryHero kind="welcome" onDiscover={openDiscover} onAdd={openAdd} />
      ) : featured ? (
        <LibraryHero
          kind="game"
          summary={featured}
          playing={playing.has(featured.game.id)}
          weekRuntimeMs={recent?.games.find((game) => game.game_id === featured.game.id)?.runtime_ms ?? 0}
        />
      ) : (
        <LibraryHero kind="unplayed" gameCount={visible.length} onDiscover={openDiscover} onAdd={openAdd} />
      )}

      {error && (
        <p role="alert" className="border-b border-rule px-8 py-3 text-sm text-amber xl:px-14">
          Could not load your library: {error}
        </p>
      )}

      {featured && recent && <RecentStats recent={recent} summaries={summaries} sessions={sessions} />}

      {shelf.length > 0 && (
        <Shelf
          games={shelf}
          playing={playing}
          totalCount={visible.length}
          onShowAll={() => document.getElementById("all-games")?.scrollIntoView({ behavior: scrollMotion() })}
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
