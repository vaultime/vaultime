// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Link } from "react-router";
import { MoreVertical, Pencil, Trash2, Zap } from "lucide-react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { GameArtwork } from "@/components/media/GameArtwork";
import { IntegrityBadge } from "@/components/status/IntegrityBadge";
import type { Game } from "@/lib/types";
import { formatCalendarDay, formatCompactDuration } from "@/lib/time";

interface GameCardProps {
  game: Game;
  detailTo: string;
  isRunning?: boolean;
  totalRuntimeMs?: number;
  totalActiveMs?: number;
  sessionCount?: number;
  lastPlayedAt?: string | null;
  coverImageUrl?: string | null;
  integrityStatus?: string;
  suspiciousCount?: number;
  recoveredCount?: number;
  onEdit: (game: Game) => void;
  onDelete: (game: Game) => void;
}

export function GameCard({
  game,
  detailTo,
  isRunning = false,
  totalRuntimeMs = 0,
  totalActiveMs = 0,
  sessionCount = 0,
  lastPlayedAt = null,
  coverImageUrl = null,
  integrityStatus = "local",
  onEdit,
  onDelete,
}: GameCardProps) {
  const hasTrackedTime = totalRuntimeMs > 0 || totalActiveMs > 0;
  const activeRatio =
    totalRuntimeMs > 0 ? Math.round((totalActiveMs / totalRuntimeMs) * 100) : 0;

  return (
    <div
      className={`group relative overflow-hidden rounded-2xl border bg-card/85 transition-all duration-300 hover:-translate-y-1 hover:shadow-[0_12px_40px_rgba(135,88,255,0.15)] ${
        isRunning
          ? "border-green-500/50 shadow-[0_0_20px_rgba(34,197,94,0.12)]"
          : "border-border/70 hover:border-primary/40"
      }`}
    >
      {/* Running glow overlay */}
      {isRunning && (
        <div className="pointer-events-none absolute inset-0 z-20 rounded-2xl border-2 border-green-500/30 animate-pulse-glow" />
      )}

      {/* Running badge */}
      {isRunning && (
        <div className="absolute top-3 left-3 z-10">
          <div className="flex items-center gap-1.5 rounded-full bg-green-500/90 px-2.5 py-1 text-[11px] font-bold uppercase tracking-wider text-white shadow-[0_0_12px_rgba(34,197,94,0.4)]">
            <span className="relative flex h-2 w-2">
              <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-white opacity-75" />
              <span className="relative inline-flex h-2 w-2 rounded-full bg-white" />
            </span>
            Live
          </div>
        </div>
      )}

      <div className="absolute top-3 right-3 z-10">
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 shrink-0 rounded-xl bg-black/40 backdrop-blur-sm opacity-0 transition-opacity group-hover:opacity-100"
              />
            }
          >
            <MoreVertical className="h-4 w-4" />
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onClick={() => onEdit(game)}>
              <Pencil className="mr-2 h-4 w-4" />
              Edit
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => onDelete(game)}
              className="text-destructive focus:text-destructive"
            >
              <Trash2 className="mr-2 h-4 w-4" />
              Delete
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <Link to={detailTo} className="block">
        <div className="relative">
          <GameArtwork
            src={coverImageUrl}
            alt={`${game.title} artwork`}
            className="h-52"
            imageClassName="transition-transform duration-500 group-hover:scale-[1.05]"
            iconClassName="h-14 w-14"
          />
          {/* Bottom gradient for text readability */}
          <div className="absolute inset-x-0 bottom-0 h-28 bg-gradient-to-t from-card via-card/80 to-transparent" />

          {/* Title overlay on artwork */}
          <div className="absolute inset-x-0 bottom-0 p-4">
            <h3 className="truncate text-base font-bold leading-tight drop-shadow-[0_2px_4px_rgba(0,0,0,0.8)]">
              {game.title}
            </h3>
            <p className="mt-0.5 truncate text-[11px] text-white/50">
              {game.executable_path
                ? game.executable_path.split(/[\\/]/).pop()
                : "Manual entry"}
            </p>
          </div>
        </div>

        <div className="space-y-3 p-4 pt-2">
          {hasTrackedTime ? (
            <>
              {/* Main stat — active time prominent */}
              <div className="flex items-baseline justify-between">
                <div className="flex items-center gap-1.5">
                  <Zap className="h-3.5 w-3.5 text-[color:var(--color-chart-1)]" />
                  <span className="text-lg font-bold tabular-nums">
                    {formatCompactDuration(totalActiveMs)}
                  </span>
                </div>
                <span className="text-xs tabular-nums text-muted-foreground">
                  {formatCompactDuration(totalRuntimeMs)} total
                </span>
              </div>

              {/* Active ratio bar */}
              <div className="relative h-1.5 overflow-hidden rounded-full bg-white/8">
                <div
                  className="h-full rounded-full bg-gradient-to-r from-[color:var(--color-chart-1)] to-[color:var(--color-chart-5)] transition-all duration-500"
                  style={{
                    width: `${Math.max(4, activeRatio)}%`,
                  }}
                />
              </div>

              {/* Bottom row — sessions, integrity, last played */}
              <div className="flex items-center justify-between text-[11px] text-muted-foreground">
                <div className="flex items-center gap-2">
                  <span>
                    {sessionCount} session{sessionCount === 1 ? "" : "s"}
                  </span>
                  <IntegrityBadge status={integrityStatus} className="text-[9px] px-1.5 py-0" />
                </div>
                <span>
                  {lastPlayedAt
                    ? formatCalendarDay(lastPlayedAt)
                    : ""}
                </span>
              </div>
            </>
          ) : (
            <div className="rounded-xl border border-dashed border-white/10 bg-white/[0.03] px-3 py-4 text-center text-xs text-muted-foreground">
              Launch to start tracking
            </div>
          )}
        </div>
      </Link>
    </div>
  );
}
