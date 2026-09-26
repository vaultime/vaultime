// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useCallback, useState } from "react";
import {
  Check,
  FolderSearch,
  Gamepad2,
  Loader2,
  Search,
} from "lucide-react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { DiscoveredGame } from "@/lib/types";
import * as api from "@/lib/tauri";

interface DiscoverGamesDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImported: () => void;
}

function sourceLabel(source: string): string {
  switch (source) {
    case "steam":
      return "Steam";
    case "folder_scan":
      return "Folder";
    default:
      return source;
  }
}

function sourceBadgeClass(source: string): string {
  switch (source) {
    case "steam":
      return "border-blue-400/50 text-blue-200";
    default:
      return "border-muted-foreground/50 text-muted-foreground";
  }
}

export function DiscoverGamesDialog({ open, onOpenChange, onImported }: DiscoverGamesDialogProps) {
  const [scanning, setScanning] = useState(false);
  const [importing, setImporting] = useState(false);
  const [results, setResults] = useState<DiscoveredGame[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanned, setScanned] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reset = useCallback(() => {
    setResults([]);
    setSelected(new Set());
    setScanned(false);
    setError(null);
    setScanning(false);
    setImporting(false);
  }, []);

  // Start over every time the dialog opens.
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) reset();
  }

  async function handleScan() {
    try {
      setScanning(true);
      setError(null);
      setResults([]);
      setSelected(new Set());

      const [steamResult, folderResult] = await Promise.allSettled([
        api.discoverSteamGames(),
        api
          .getDefaultScanPaths()
          .then((paths) => (paths.length > 0 ? api.discoverGames(paths) : [])),
      ]);

      const failures: string[] = [];
      if (steamResult.status === "rejected") {
        failures.push(`Steam scan failed: ${String(steamResult.reason)}`);
      }
      if (folderResult.status === "rejected") {
        failures.push(`Folder scan failed: ${String(folderResult.reason)}`);
      }
      setError(failures.length > 0 ? failures.join(" ") : null);

      const steamGames =
        steamResult.status === "fulfilled" ? steamResult.value : [];
      const folderGames =
        folderResult.status === "fulfilled" ? folderResult.value : [];

      // Steam entries win over folder matches for the same executable.
      const byPath = new Map<string, DiscoveredGame>();
      for (const game of [...steamGames, ...folderGames]) {
        if (!byPath.has(game.executable_path)) {
          byPath.set(game.executable_path, game);
        }
      }

      const merged = Array.from(byPath.values());
      merged.sort((a, b) => {
        if (a.already_added !== b.already_added) {
          return a.already_added ? 1 : -1;
        }
        return a.title.localeCompare(b.title);
      });

      setResults(merged);

      // Folder scan hits are guesses, so only Steam results start selected.
      setSelected(
        new Set(
          merged
            .filter((g) => !g.already_added && g.source === "steam")
            .map((g) => g.executable_path),
        ),
      );
      setScanned(true);
    } catch (scanError) {
      setError(String(scanError));
    } finally {
      setScanning(false);
    }
  }

  function toggleSelection(exePath: string) {
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(exePath)) {
        next.delete(exePath);
      } else {
        next.add(exePath);
      }
      return next;
    });
  }

  function selectAll() {
    setSelected(
      new Set(
        results
          .filter((g) => !g.already_added)
          .map((g) => g.executable_path),
      ),
    );
  }

  function selectNone() {
    setSelected(new Set());
  }

  async function handleImport() {
    const toImport = results.filter(
      (g) => !g.already_added && selected.has(g.executable_path),
    );
    if (toImport.length === 0) return;

    try {
      setImporting(true);
      setError(null);
      await api.importDiscoveredGames(toImport);
      onOpenChange(false);
      onImported();
    } catch (importError) {
      setError(String(importError));
    } finally {
      setImporting(false);
    }
  }

  const newCount = results.filter((g) => !g.already_added).length;
  const selectedCount = selected.size;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>

      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <FolderSearch className="h-4 w-4 text-primary" />
            Discover games
          </DialogTitle>
          <DialogDescription>
            Scan common install folders and Steam for games to add to your
            library.
          </DialogDescription>
        </DialogHeader>

        {error && (
          <div className="rounded-2xl border border-destructive/40 bg-destructive/10 p-3 text-sm text-destructive">
            {error}
          </div>
        )}

        {!scanned ? (
          <div className="flex flex-col items-center gap-4 py-8">
            <div className="rounded-2xl border border-dashed border-border/70 bg-muted/15 p-8 text-center">
              <FolderSearch className="mx-auto h-10 w-10 text-muted-foreground/50" />
              <p className="mt-3 text-sm text-muted-foreground">
                Vaultime will scan common game directories and your Steam
                library for installed games.
              </p>
            </div>
            <Button onClick={handleScan} disabled={scanning}>
              {scanning ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Search className="h-4 w-4" />
              )}
              {scanning ? "Scanning..." : "Start Scan"}
            </Button>
          </div>
        ) : results.length === 0 ? (
          <div className="rounded-2xl border border-dashed border-border/70 bg-muted/15 p-8 text-center">
            <p className="text-sm text-muted-foreground">
              No new games found. Try adding games manually or check your install
              folders.
            </p>
          </div>
        ) : (
          <div className="space-y-3">
            <div className="flex items-center justify-between">
              <p className="text-sm text-muted-foreground">
                Found {results.length} game{results.length === 1 ? "" : "s"}
                {newCount < results.length &&
                  ` (${results.length - newCount} already in library)`}
              </p>
              <div className="flex gap-2">
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={selectAll}
                  disabled={newCount === 0}
                >
                  All
                </Button>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={selectNone}
                >
                  None
                </Button>
              </div>
            </div>

            <div className="max-h-80 space-y-1.5 overflow-y-auto rounded-2xl border border-border/70 bg-muted/10 p-2">
              {results.map((game) => {
                const isSelected = selected.has(game.executable_path);
                const disabled = game.already_added;

                return (
                  <button
                    key={game.executable_path}
                    type="button"
                    className={`flex w-full items-start gap-3 rounded-xl px-3 py-2.5 text-left transition-colors ${
                      disabled
                        ? "cursor-default opacity-50"
                        : isSelected
                          ? "bg-primary/15 hover:bg-primary/20"
                          : "hover:bg-muted/30"
                    }`}
                    onClick={() => {
                      if (!disabled) toggleSelection(game.executable_path);
                    }}
                    disabled={disabled}
                  >
                    <div
                      className={`mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-md border ${
                        disabled
                          ? "border-muted-foreground/30 bg-muted/20"
                          : isSelected
                            ? "border-primary bg-primary text-primary-foreground"
                            : "border-border/70 bg-background/50"
                      }`}
                    >
                      {(isSelected || disabled) && (
                        <Check className="h-3 w-3" />
                      )}
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <Gamepad2 className="h-3.5 w-3.5 shrink-0 text-muted-foreground" />
                        <span className="truncate text-sm font-medium">
                          {game.title}
                        </span>
                        <Badge
                          variant="outline"
                          className={`shrink-0 text-[10px] ${sourceBadgeClass(game.source)}`}
                        >
                          {sourceLabel(game.source)}
                        </Badge>
                        {game.already_added && (
                          <Badge
                            variant="outline"
                            className="shrink-0 border-emerald-500/50 text-[10px] text-emerald-300"
                          >
                            Added
                          </Badge>
                        )}
                      </div>
                      <p className="mt-0.5 truncate text-xs text-muted-foreground">
                        {game.executable_path}
                      </p>
                    </div>
                  </button>
                );
              })}
            </div>
          </div>
        )}

        <DialogFooter>
          <DialogClose render={<Button variant="ghost" />}>
            Cancel
          </DialogClose>
          {scanned && newCount > 0 && (
            <Button
              onClick={handleImport}
              disabled={importing || selectedCount === 0}
            >
              {importing ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Gamepad2 className="h-4 w-4" />
              )}
              {importing
                ? "Importing..."
                : `Import ${selectedCount} Game${selectedCount === 1 ? "" : "s"}`}
            </Button>
          )}
          {scanned && (
            <Button variant="outline" onClick={handleScan} disabled={scanning}>
              {scanning ? (
                <Loader2 className="h-4 w-4 animate-spin" />
              ) : (
                <Search className="h-4 w-4" />
              )}
              Rescan
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
