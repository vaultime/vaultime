// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { Outlet } from "react-router";
import { Backdrop } from "@/features/appearance/Backdrop";
import { CommandPalette } from "./CommandPalette";
import { LiveBar } from "./LiveBar";
import { Rail } from "./Rail";
import { UpdateBanner } from "./UpdateBanner";

export function AppLayout() {
  const [paletteOpen, setPaletteOpen] = useState(false);

  useEffect(() => {
    function onKeyDown(event: KeyboardEvent) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setPaletteOpen((open) => !open);
      }
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="relative isolate grid h-screen grid-cols-[240px_minmax(0,1fr)] xl:grid-cols-[288px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_auto] overflow-hidden bg-ink text-text">
      <Backdrop />
      <Rail onSearch={() => setPaletteOpen(true)} />
      <main className="min-h-0 overflow-y-auto">
        <UpdateBanner />
        <Outlet />
      </main>
      <LiveBar />
      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} />
    </div>
  );
}
