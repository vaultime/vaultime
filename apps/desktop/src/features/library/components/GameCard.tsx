// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { Gamepad2, MoreVertical, Pencil, Trash2 } from "lucide-react";
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
import type { Game } from "@/lib/types";

interface GameCardProps {
  game: Game;
  isRunning?: boolean;
  totalPlaytimeMs?: number;
  onEdit: (game: Game) => void;
  onDelete: (game: Game) => void;
}

function formatPlaytime(ms: number): string {
  const totalMinutes = Math.floor(ms / 60_000);
  const hours = Math.floor(totalMinutes / 60);
  const minutes = totalMinutes % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  return `${minutes}m`;
}

export function GameCard({
  game,
  isRunning = false,
  totalPlaytimeMs = 0,
  onEdit,
  onDelete,
}: GameCardProps) {
  return (
    <Card className="group relative overflow-hidden transition-colors hover:border-primary/40">
      {/* Running indicator */}
      {isRunning && (
        <div className="absolute top-2 left-2 z-10">
          <Badge
            variant="default"
            className="bg-green-600 text-white"
          >
            Running
          </Badge>
        </div>
      )}

      {/* Cover art placeholder */}
      <div className="flex h-40 items-center justify-center bg-muted">
        <Gamepad2 className="h-12 w-12 text-muted-foreground/30" />
      </div>

      <CardContent className="p-3">
        <div className="flex items-start justify-between gap-2">
          <div className="min-w-0 flex-1">
            <h3 className="truncate text-sm font-semibold leading-tight">
              {game.title}
            </h3>
            <p className="mt-1 truncate text-xs text-muted-foreground">
              {totalPlaytimeMs > 0
                ? formatPlaytime(totalPlaytimeMs)
                : game.executable_path
                  ? game.executable_path.split(/[\\/]/).pop()
                  : "No executable"}
            </p>
          </div>

          <DropdownMenu>
            <DropdownMenuTrigger
              render={
                <Button
                  variant="ghost"
                  size="icon"
                  className="h-7 w-7 shrink-0 opacity-0 group-hover:opacity-100"
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
      </CardContent>
    </Card>
  );
}
