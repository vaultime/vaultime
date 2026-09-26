// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useEffect, useState } from "react";
import { NavLink } from "react-router";
import {
  Clock,
  Cloud,
  Gamepad2,
  Settings,
} from "lucide-react";
import { Logo } from "@/components/brand/Logo";
import { useCloudSession } from "@/features/cloud/CloudSessionProvider";
import { cn } from "@/lib/utils";
import * as api from "@/lib/tauri";

const navItems = [
  { to: "/library", label: "Library", icon: Gamepad2 },
  { to: "/sessions", label: "Sessions", icon: Clock },
  { to: "/cloud", label: "Cloud", icon: Cloud },
  { to: "/settings", label: "Settings", icon: Settings },
] as const;

export function Sidebar() {
  const [appVersion, setAppVersion] = useState("0.1.0");
  const { session, isAdmin } = useCloudSession();

  useEffect(() => {
    api.getAppVersion().then(setAppVersion).catch(() => {});
  }, []);

  return (
    <aside className="relative m-4 flex h-[calc(100%-2rem)] w-64 shrink-0 flex-col overflow-hidden rounded-[2rem] border border-white/10 bg-[linear-gradient(180deg,rgba(31,20,53,0.92),rgba(16,10,29,0.94))] shadow-[0_24px_80px_rgba(7,3,18,0.45)] backdrop-blur-xl">
      <div className="absolute inset-0 bg-[radial-gradient(circle_at_top,rgba(150,104,255,0.2),transparent_32%),radial-gradient(circle_at_bottom,rgba(87,41,174,0.24),transparent_40%)]" />

      <div className="relative flex h-20 items-center gap-3 px-5">
        <Logo />
      </div>

      <nav className="relative flex-1 space-y-1 px-3 py-2">
        {navItems.map(({ to, label, icon: Icon }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              cn(
                "group/nav relative flex items-center gap-3 rounded-2xl px-3.5 py-3 text-sm font-medium transition-all duration-200",
                isActive
                  ? "bg-white/10 text-sidebar-primary shadow-[inset_0_1px_0_rgba(255,255,255,0.08),0_0_20px_rgba(135,88,255,0.1)]"
                  : "text-sidebar-foreground/70 hover:bg-white/6 hover:text-sidebar-foreground",
              )
            }
          >
            {({ isActive }) => (
              <>
                {isActive && (
                  <span className="absolute left-0 top-1/2 h-6 w-[3px] -translate-y-1/2 rounded-r-full bg-sidebar-primary shadow-[0_0_8px_rgba(135,88,255,0.6)] animate-fade-up" />
                )}
                <Icon className={cn(
                  "h-4 w-4 transition-transform duration-200",
                  isActive && "scale-110",
                )} />
                <span className="flex items-center gap-2">
                  {label}
                  {to === "/cloud" && session && (
                    <span
                      className={cn(
                        "h-2 w-2 rounded-full",
                        isAdmin
                          ? "bg-amber-300 shadow-[0_0_10px_rgba(252,211,77,0.85)]"
                          : "bg-emerald-300 shadow-[0_0_10px_rgba(110,231,183,0.85)]",
                      )}
                    />
                  )}
                </span>
              </>
            )}
          </NavLink>
        ))}
      </nav>

      <div className="relative border-t border-white/8 px-5 py-4">
        <div className="flex items-center gap-2">
          <p className="text-[11px] uppercase tracking-[0.28em] text-sidebar-foreground/45">
            Vaultime
          </p>
          <span className="rounded-full border border-white/10 bg-white/[0.04] px-2 py-0.5 text-[10px] tabular-nums text-sidebar-foreground/40">
            v{appVersion}
          </span>
        </div>
        <p className="mt-2 text-xs leading-5 text-sidebar-foreground/55">
          {session
            ? `Cloud connected as ${session.user.role}.`
            : "Local-first tracking with active and runtime history."}
        </p>
      </div>
    </aside>
  );
}
