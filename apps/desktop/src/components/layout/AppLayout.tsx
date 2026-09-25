// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Outlet } from "react-router";
import { Sidebar } from "./Sidebar";

export function AppLayout() {
  return (
    <div className="relative flex h-screen overflow-hidden bg-background text-foreground">
      <div className="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_top_left,rgba(135,88,255,0.22),transparent_28%),radial-gradient(circle_at_80%_10%,rgba(86,50,170,0.24),transparent_24%),radial-gradient(circle_at_bottom_right,rgba(53,26,105,0.34),transparent_34%),linear-gradient(180deg,rgba(11,7,18,0.96),rgba(7,5,14,1))]" />
      <Sidebar />
      <main className="relative flex-1 overflow-y-auto">
        <div className="min-h-full p-6 lg:p-8">
          <Outlet />
        </div>
      </main>
    </div>
  );
}
