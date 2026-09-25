// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Link } from "react-router";
import { MoreVertical, Pencil, TimerReset, Trash2 } from "lucide-react";
import {
  Card,
  CardContent,
} from "@/components/ui/card";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { Button } from "@/components/ui/button";
import { Badge } from "@/components/ui/badge";
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
  suspiciousCount = 0,
  recoveredCount = 0,
  onEdit,
  onDelete,
}: GameCardProps) {
  const hasTrackedTime = totalRuntimeMs > 0 || totalActiveMs > 0;
  const runtimeDiffers = totalRuntimeMs > totalActiveMs;
  const integrityHint =
    suspiciousCount > 0
      ? `${suspiciousCount} flagged`
      : recoveredCount > 0
        ? `${recoveredCount} recovered`
        : "Local history";

  return (
    <Card className="group relative overflow-hidden border border-border/70 bg-card/85 transition-colors hover:border-primary/40">
      {/* Running indicator */}
      {isRunning && (
        <div className="absolute top-3 left-3 z-10">
          <Badge
            variant="default"
            className="bg-green-600 text-white"
          >
            Running
          </Badge>
        </div>
      )}

      <div className="absolute top-3 right-3 z-10">
        <DropdownMenu>
          <DropdownMenuTrigger
            render={
              <Button
                variant="ghost"
                size="icon"
                className="h-8 w-8 shrink-0 bg-background/70 backdrop-blur-sm opacity-0 group-hover:opacity-100"
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

      <Link
        to={detailTo}
        className="block transition-transform group-hover:-translate-y-0.5"
      >
        <GameArtwork
          src={coverImageUrl}
          alt={`${game.title} artwork`}
          className="h-48"
          imageClassName="transition-transform duration-500 group-hover:scale-[1.03]"
          iconClassName="h-14 w-14"
        />
        <div className="relative -mt-24 flex h-24 items-end justify-center overflow-hidden">
          <div className="absolute inset-x-0 bottom-0 h-24 bg-gradient-to-t from-background/65 to-transparent" />
        </div>

        <CardContent className="relative p-4 pt-0">
          <div className="space-y-3">
            <div className="min-w-0">
              <h3 className="truncate text-base font-semibold leading-tight">
                {game.title}
              </h3>
              <p className="mt-1 truncate text-xs text-muted-foreground">
                {game.executable_path
                  ? game.executable_path.split(/[\\/]/).pop()
                  : "Manual library entry"}
              </p>
              {sessionCount > 0 && (
                <div className="mt-3 flex flex-wrap items-center gap-2">
                  <IntegrityBadge status={integrityStatus} />
                  <span className="text-[11px] text-muted-foreground">
                    {integrityHint}
                  </span>
                </div>
              )}
            </div>

            {hasTrackedTime ? (
              <div className="grid grid-cols-2 gap-2">
                <div className="rounded-2xl border border-border/70 bg-muted/25 p-3">
                  <p className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                    Active
                  </p>
                  <p className="mt-1 text-sm font-semibold">
                    {formatCompactDuration(totalActiveMs)}
                  </p>
                </div>
                <div className="rounded-2xl border border-border/70 bg-muted/25 p-3">
                  <p className="text-[10px] uppercase tracking-[0.18em] text-muted-foreground">
                    Runtime
                  </p>
                  <p className="mt-1 text-sm font-semibold">
                    {formatCompactDuration(totalRuntimeMs)}
                  </p>
                </div>
              </div>
            ) : (
              <div className="rounded-2xl border border-dashed border-border/70 bg-muted/15 p-3 text-xs text-muted-foreground">
                No tracked sessions yet. Launch the game once to start building history.
              </div>
            )}

            <div className="flex items-center justify-between gap-3 text-xs text-muted-foreground">
              <div className="flex items-center gap-1.5">
                <TimerReset className="h-3.5 w-3.5" />
                <span>
                  {sessionCount} session{sessionCount === 1 ? "" : "s"}
                </span>
              </div>
              <span>
                {lastPlayedAt ? `Last played ${formatCalendarDay(lastPlayedAt)}` : "Never launched"}
              </span>
            </div>

            {runtimeDiffers && hasTrackedTime && (
              <div className="h-2 overflow-hidden rounded-full bg-muted">
                <div
                  className="h-full rounded-full bg-[color:var(--color-chart-1)]"
                  style={{
                    width: `${Math.max(4, (totalActiveMs / totalRuntimeMs) * 100)}%`,
                  }}
                />
              </div>
            )}
          </div>
        </CardContent>
      </Link>
    </Card>
  );
}
