// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { NavLink } from "react-router";
import {
  Gamepad2,
  Clock,
  Cloud,
  Settings,
} from "lucide-react";
import { cn } from "@/lib/utils";

const navItems = [
  { to: "/library", label: "Library", icon: Gamepad2 },
  { to: "/sessions", label: "Sessions", icon: Clock },
  { to: "/cloud", label: "Cloud", icon: Cloud },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

export function Sidebar() {
  return (
    <aside className="relative m-4 flex h-[calc(100%-2rem)] w-64 shrink-0 flex-col overflow-hidden rounded-[2rem] border border-white/10 bg-[linear-gradient(180deg,rgba(31,20,53,0.92),rgba(16,10,29,0.94))] shadow-[0_24px_80px_rgba(7,3,18,0.45)] backdrop-blur-xl">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(150,104,255,0.2),transparent_32%),radial-gradient(circle_at_bottom,rgba(87,41,174,0.24),transparent_40%)]" />
      <div className="relative flex h-20 items-center gap-3 px-5">
        <div className="flex h-11 w-11 items-center justify-center rounded-2xl bg-white/8 ring-1 ring-white/10">
          <Gamepad2 className="h-5 w-5 text-sidebar-primary" />
        </div>
        <div>
          <span className="block text-lg font-semibold tracking-tight text-sidebar-foreground">
            Vaultime
          </span>
          <span className="block text-xs text-sidebar-foreground/55">
            Deep local play archive
          </span>
        </div>
      </div>

      <nav className="relative flex-1 space-y-1.5 px-3 py-2">
        {navItems.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-3 rounded-2xl px-3.5 py-3 text-sm font-medium transition-all",
                isActive
                  ? "bg-white/10 text-sidebar-primary shadow-[inset_0_1px_0_rgba(255,255,255,0.08)]"
                  : "text-sidebar-foreground/70 hover:bg-white/6 hover:text-sidebar-foreground",
              )
            }
          >
            <Icon className="h-4 w-4" />
            {label}
          </NavLink>
        ))}
      </nav>

      <div className="relative border-t border-white/8 px-5 py-4">
        <p className="text-[11px] uppercase tracking-[0.28em] text-sidebar-foreground/45">
          Vaultime v0.1.0
        </p>
        <p className="mt-2 text-xs leading-5 text-sidebar-foreground/55">
          Local-first tracking with active and runtime history.
        </p>
      </div>
    </aside>
  );
}
