// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useState } from "react";
import type { ActivityPoint } from "@/lib/session-stats";
import { cn } from "@/lib/utils";
import { formatCompactDuration } from "@/lib/time";

interface ActivityChartProps {
  points: ActivityPoint[];
  className?: string;
  compact?: boolean;
}

export function ActivityChart({
  points,
  className,
  compact = false,
}: ActivityChartProps) {
  const [hovered, setHovered] = useState<string | null>(null);
  const maxRuntime = Math.max(1, ...points.map((p) => p.runtimeMs));

  return (
    <div
      className={cn(
        "grid grid-cols-[repeat(auto-fit,minmax(18px,1fr))] items-end gap-1.5",
        compact ? "h-32" : "h-48",
        className,
      )}
    >
      {points.map((point) => {
        const hasRuntime = point.runtimeMs > 0;
        const runtimePct = (point.runtimeMs / maxRuntime) * 100;
        const activeRatio =
          point.runtimeMs > 0 ? point.activeMs / point.runtimeMs : 0;
        const activePct = Math.max(0, activeRatio * 100);
        const isHovered = hovered === point.key;

        return (
          <div
            key={point.key}
            className="group flex min-w-0 flex-col items-center gap-1.5"
            onMouseEnter={() => setHovered(point.key)}
            onMouseLeave={() => setHovered(null)}
          >
            {/* Tooltip */}
            {isHovered && hasRuntime && (
              <div className="pointer-events-none absolute -top-1 left-1/2 z-10 -translate-x-1/2 -translate-y-full rounded-xl border border-white/10 bg-black/80 px-3 py-2 text-[11px] shadow-[0_8px_24px_rgba(0,0,0,0.4)] backdrop-blur-md">
                <p className="font-bold text-foreground">{point.label}</p>
                <div className="mt-1 flex items-center gap-2">
                  <span className="h-2 w-2 rounded-full bg-[color:var(--color-chart-1)]" />
                  <span className="text-muted-foreground">
                    {formatCompactDuration(point.activeMs)} active
                  </span>
                </div>
                <div className="flex items-center gap-2">
                  <span className="h-2 w-2 rounded-full bg-[color:var(--color-chart-3)]/60" />
                  <span className="text-muted-foreground">
                    {formatCompactDuration(point.runtimeMs)} runtime
                  </span>
                </div>
              </div>
            )}

            {/* Bar */}
            <div className="relative flex h-full w-full items-end">
              <div
                className={cn(
                  "relative w-full overflow-hidden rounded-xl transition-all duration-300",
                  hasRuntime
                    ? "bg-white/[0.04]"
                    : "bg-white/[0.02]",
                  isHovered && hasRuntime && "shadow-[0_0_16px_rgba(135,88,255,0.25)]",
                )}
                style={{ height: hasRuntime ? `${Math.max(6, runtimePct)}%` : "4%" }}
              >
                {hasRuntime && (
                  <>
                    {/* Runtime layer */}
                    <div
                      className={cn(
                        "absolute inset-0 rounded-xl bg-[color:var(--color-chart-3)]/30 transition-all duration-300",
                        isHovered && "bg-[color:var(--color-chart-3)]/45",
                      )}
                    />
                    {/* Active layer — gradient fill from bottom */}
                    <div
                      className={cn(
                        "absolute inset-x-0 bottom-0 rounded-b-xl bg-gradient-to-t from-[color:var(--color-chart-1)] to-[color:var(--color-chart-5)] transition-all duration-300",
                        isHovered ? "opacity-100" : "opacity-85",
                      )}
                      style={{ height: `${activePct}%` }}
                    />
                    {/* Glow line at top of active section */}
                    {activePct > 10 && (
                      <div
                        className="absolute inset-x-1 h-px bg-white/20 transition-opacity duration-300"
                        style={{ bottom: `${activePct}%` }}
                      />
                    )}
                  </>
                )}
              </div>
            </div>

            {/* Date label */}
            <div className="w-full text-center">
              <p
                className={cn(
                  "truncate text-[10px] font-medium transition-colors duration-200",
                  isHovered
                    ? "text-foreground"
                    : "text-muted-foreground",
                )}
              >
                {compact ? point.key.slice(8).replace(/^0/, "") : point.label}
              </p>
            </div>
          </div>
        );
      })}
    </div>
  );
}
