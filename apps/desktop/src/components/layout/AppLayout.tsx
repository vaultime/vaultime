// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { Outlet } from "react-router";
import { Backdrop } from "@/features/appearance/Backdrop";
import { requestFind } from "@/lib/find";
import { CommandPalette } from "./CommandPalette";
import { LiveBar } from "./LiveBar";
import { Rail } from "./Rail";
import { UpdateBanner } from "./UpdateBanner";

export function AppLayout() {
  const [paletteOpen, setPaletteOpen] = useState(false);

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
    <div className="relative isolate grid h-screen grid-cols-[240px_minmax(0,1fr)] xl:grid-cols-[288px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_auto] overflow-hidden bg-ink text-text">
      <Backdrop />
      <Rail onSearch={() => setPaletteOpen(true)} />
      {/* Relative, so hidden inputs and other positioned content stay inside the
          scroller. Outside it they stretch the shell, and focusing one scrolls
          the whole app up. */}
      <main className="relative min-h-0 overflow-y-auto">
        <UpdateBanner />
        <Outlet />
      </main>
      <LiveBar />
      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />
    </div>
  );
}
