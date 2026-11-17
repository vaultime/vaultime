// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { Outlet, useSearchParams } from "react-router";
import { Backdrop } from "@/features/appearance/Backdrop";
import { DiscoverGamesDialog } from "@/features/library/components/DiscoverGamesDialog";
import { useLibrary } from "@/features/library/library-context";
import { useFoundGames } from "@/features/library/use-found-games";
import { requestFind } from "@/lib/find";
import { otherDialogOpen } from "@/lib/found-games";
import { CommandPalette } from "./CommandPalette";
import { LiveBar } from "./LiveBar";
import { Rail } from "./Rail";
import { TopBar } from "./TopBar";
import { UpdateBanner } from "./UpdateBanner";

export function AppLayout() {
  const [paletteOpen, setPaletteOpen] = useState(false);
  const [scrolled, setScrolled] = useState(false);
  const [searchParams] = useSearchParams();
  const { refresh } = useLibrary();
  const { found, dismiss } = useFoundGames(otherDialogOpen(searchParams, paletteOpen));

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if (!(event.ctrlKey || event.metaKey) || event.altKey || event.shiftKey) return;
      const key = event.key.toLowerCase();
      if (key === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      } else if (key === "f") {
        // The page's own search, or the palette where a page has none.
        event.preventDefault();
        if (!requestFind()) setPaletteOpen(true);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="relative isolate grid h-screen grid-cols-[248px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_auto] gap-2 overflow-hidden bg-window p-2 text-text 2xl:grid-cols-[272px_minmax(0,1fr)]">
      <Backdrop />
      <Rail />
      <div className="panel flex min-h-0 flex-col">
        {/* Above the scroller, where a tinted header reaching up under the top bar cannot cover it. */}
        <UpdateBanner />
        {/* Relative, so hidden inputs and other positioned content stay inside the
            scroller. Outside it they stretch the shell, and focusing one scrolls
            the whole app up. */}
        <main
          className="relative min-h-0 flex-1 overflow-y-auto"
          onScroll={(event) => setScrolled(event.currentTarget.scrollTop > 0)}
        >
          <TopBar scrolled={scrolled} onSearch={() => setPaletteOpen(true)} />
          <Outlet />
        </main>
      </div>
      <LiveBar />
      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />
      <DiscoverGamesDialog
        open={found !== null}
        found={found}
        onOpenChange={(open) => {
          if (!open) dismiss();
        }}
        onImported={() => void refresh()}
      />
    </div>
  );
}
