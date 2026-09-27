// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useState } from "react";
import { Outlet } from "react-router";
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
    <div className="grid h-screen grid-cols-[232px_minmax(0,1fr)] xl:grid-cols-[272px_minmax(0,1fr)] grid-rows-[minmax(0,1fr)_auto] overflow-hidden bg-ink text-text">
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
