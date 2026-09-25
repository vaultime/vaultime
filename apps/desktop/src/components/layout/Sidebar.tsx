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
    <aside className="flex h-full w-56 flex-col border-r border-sidebar-border bg-sidebar">
      <div className="flex h-14 items-center gap-2 px-4">
        <Gamepad2 className="h-6 w-6 text-sidebar-primary" />
        <span className="text-lg font-semibold tracking-tight text-sidebar-foreground">
          Vaultime
        </span>
      </div>

      <nav className="flex-1 space-y-1 px-2 py-2">
        {navItems.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-3 rounded-md px-3 py-2 text-sm font-medium transition-colors",
                isActive
                  ? "bg-sidebar-accent text-sidebar-primary"
                  : "text-sidebar-foreground/70 hover:bg-sidebar-accent hover:text-sidebar-foreground",
              )
            }
          >
            <Icon className="h-4 w-4" />
            {label}
          </NavLink>
        ))}
      </nav>

      <div className="border-t border-sidebar-border px-4 py-3">
        <p className="text-xs text-muted-foreground">Vaultime v0.1.0</p>
      </div>
    </aside>
  );
}
