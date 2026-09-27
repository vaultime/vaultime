// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useState } from "react";
import { Check, Loader2, Search } from "lucide-react";
import { Notice } from "@/components/layout/Page";
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
import * as api from "@/lib/tauri";
import type { DiscoveredGame } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";
import { plural } from "@/lib/words";

const SOURCE_LABELS: Record<string, string> = {
  steam: "Steam",
  epic: "Epic Games",
  gog: "GOG",
  heroic: "Heroic",
  lutris: "Lutris",
  folder_scan: "Folder",
};

export function DiscoverGamesDialog({
  open,
  onOpenChange,
  onImported,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImported: () => void;
}) {
  const [scanning, setScanning] = useState(false);
  const [importing, setImporting] = useState(false);
  const [results, setResults] = useState<DiscoveredGame[]>([]);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [scanned, setScanned] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Start over every time the dialog opens.
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) {
      setResults([]);
      setSelected(new Set());
      setScanned(false);
      setError(null);
      setScanning(false);
      setImporting(false);
    }
  }

  async function handleScan() {
    try {
      setScanning(true);
      setError(null);
      setResults([]);
      setSelected(new Set());

      const [steamResult, launcherResult, folderResult] = await Promise.allSettled([
        api.discoverSteamGames(),
        api.discoverLauncherGames(),
        api.getDefaultScanPaths().then((paths) => (paths.length > 0 ? api.discoverGames(paths) : [])),
      ]);

      const failures: string[] = [];
      if (steamResult.status === "rejected") failures.push(`The Steam scan failed: ${describeError(steamResult.reason)}`);
      if (launcherResult.status === "rejected") failures.push(`Reading other launchers failed: ${describeError(launcherResult.reason)}`);
      if (folderResult.status === "rejected") failures.push(`The folder scan failed: ${describeError(folderResult.reason)}`);
      setError(failures.length > 0 ? failures.join(" ") : null);

      const steamGames = steamResult.status === "fulfilled" ? steamResult.value : [];
      const launcherGames = launcherResult.status === "fulfilled" ? launcherResult.value : [];
      const folderGames = folderResult.status === "fulfilled" ? folderResult.value : [];

      // Launcher entries win over folder matches for the same executable.
      const byPath = new Map<string, DiscoveredGame>();
      for (const game of [...steamGames, ...launcherGames, ...folderGames]) {
        if (!byPath.has(game.executable_path)) byPath.set(game.executable_path, game);
      }
      const merged = [...byPath.values()].sort((a, b) =>
        a.already_added !== b.already_added ? (a.already_added ? 1 : -1) : a.title.localeCompare(b.title),
      );
      setResults(merged);

      // Folder scan hits are guesses, so only launcher results start selected.
      setSelected(
        new Set(
          merged
            .filter((game) => !game.already_added && game.source !== "folder_scan")
            .map((game) => game.executable_path),
        ),
      );
      setScanned(true);
    } catch (scanError) {
      setError(describeError(scanError));
    } finally {
      setScanning(false);
    }
  }

  function toggle(path: string) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  }

  async function handleImport() {
    const toImport = results.filter((game) => !game.already_added && selected.has(game.executable_path));
    if (toImport.length === 0) return;
    try {
      setImporting(true);
      setError(null);
      await api.importDiscoveredGames(toImport);
      onOpenChange(false);
      onImported();
    } catch (importError) {
      setError(describeError(importError));
    } finally {
      setImporting(false);
    }
  }

  const newCount = results.filter((game) => !game.already_added).length;
  const selectedCount = selected.size;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Discover games</DialogTitle>
          <DialogDescription>
            Reads Steam, Epic, GOG Galaxy, Heroic and Lutris, and looks through the usual install folders on every drive.
          </DialogDescription>
        </DialogHeader>

        {error && <Notice tone="warning">{error}</Notice>}

        {!scanned ? (
          <div className="py-2">
            <Button onClick={handleScan} disabled={scanning}>
              {scanning ? <Loader2 className="size-4 animate-spin" /> : <Search className="size-4" />}
              {scanning ? "Scanning" : "Start the scan"}
            </Button>
          </div>
        ) : results.length === 0 ? (
          <p className="text-sm text-faint">No games found. You can still add any game by hand.</p>
        ) : (
          <div>
            <div className="flex items-center justify-between pb-2">
              <p className="text-sm text-faint">
                {plural(results.length, "game")} found
                {newCount < results.length && `, ${results.length - newCount} already in your library`}
              </p>
              <div className="flex gap-1">
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={newCount === 0}
                  onClick={() =>
                    setSelected(
                      new Set(results.filter((game) => !game.already_added).map((game) => game.executable_path)),
                    )
                  }
                >
                  Select all
                </Button>
                <Button variant="ghost" size="sm" onClick={() => setSelected(new Set())}>
                  Select none
                </Button>
              </div>
            </div>

            <ul className="max-h-80 overflow-y-auto border-y border-rule">
              {results.map((game) => {
                const isSelected = selected.has(game.executable_path);
                const added = game.already_added;
                return (
                  <li key={game.executable_path} className="border-b border-rule last:border-b-0">
                    <button
                      type="button"
                      aria-pressed={added || isSelected}
                      disabled={added}
                      onClick={() => toggle(game.executable_path)}
                      className="flex w-full items-center gap-3 px-1 py-2.5 text-left transition-colors hover:bg-raised/60 focus-visible:bg-raised focus-visible:outline-none disabled:opacity-50 disabled:hover:bg-transparent"
                    >
                      <span
                        aria-hidden="true"
                        className={cn(
                          "flex size-5 shrink-0 items-center justify-center rounded-md border",
                          added || isSelected ? "border-violet bg-violet text-violet-ink" : "border-hairline",
                        )}
                      >
                        {(added || isSelected) && <Check className="size-3.5" strokeWidth={2.5} />}
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="flex items-baseline gap-2">
                          <span className="truncate text-sm text-text">{game.title}</span>
                          <span className="label-caps shrink-0">
                            {added ? "In library" : (SOURCE_LABELS[game.source] ?? game.source)}
                          </span>
                        </span>
                        <span className="block truncate font-mono text-[11px] text-faint">{game.executable_path}</span>
                      </span>
                    </button>
                  </li>
                );
              })}
            </ul>
          </div>
        )}

        <DialogFooter>
          <DialogClose render={<Button variant="ghost" />}>Cancel</DialogClose>
          {scanned && (
            <Button variant="outline" onClick={handleScan} disabled={scanning}>
              {scanning ? <Loader2 className="size-4 animate-spin" /> : <Search className="size-4" />}
              Scan again
            </Button>
          )}
          {scanned && newCount > 0 && (
            <Button onClick={handleImport} disabled={importing || selectedCount === 0}>
              {importing && <Loader2 className="size-4 animate-spin" />}
              {importing ? "Adding" : `Add ${plural(selectedCount, "game")}`}
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
