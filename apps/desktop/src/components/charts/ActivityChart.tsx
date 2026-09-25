// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

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
  const maxRuntime = Math.max(
    1,
    ...points.map((point) => point.runtimeMs),
  );

  return (
    <div
      className={cn(
        "grid grid-cols-[repeat(auto-fit,minmax(18px,1fr))] items-end gap-2",
        compact ? "h-32" : "h-48",
        className,
      )}
    >
      {points.map((point) => {
        const hasRuntime = point.runtimeMs > 0;
        const runtimeHeight = `${(point.runtimeMs / maxRuntime) * 100}%`;
        const activeRatio =
          point.runtimeMs > 0 ? point.activeMs / point.runtimeMs : 0;
        const activeHeight = `${Math.max(0, activeRatio * 100)}%`;

        return (
          <div
            key={point.key}
            className="group flex min-w-0 flex-col items-center gap-2"
            title={`${point.label}: ${formatCompactDuration(
              point.activeMs,
            )} active / ${formatCompactDuration(point.runtimeMs)} runtime`}
          >
            <div className="relative flex h-full w-full items-end">
              <div className="relative w-full overflow-hidden rounded-2xl border border-border/70 bg-muted/50">
                {hasRuntime && (
                  <div
                    className="ml-auto w-full rounded-t-2xl bg-[color:var(--color-chart-3)]/55 transition-opacity group-hover:opacity-80"
                    style={{ height: runtimeHeight }}
                  >
                    <div
                      className="w-full bg-[color:var(--color-chart-1)] transition-opacity group-hover:opacity-90"
                      style={{ height: activeHeight }}
                    />
                  </div>
                )}
              </div>
            </div>
            <div className="w-full text-center">
              <p className="truncate text-[10px] font-medium text-muted-foreground">
                {compact ? point.key.slice(8).replace(/^0/, "") : point.label}
              </p>
            </div>
          </div>
        );
      })}
    </div>
  );
}
