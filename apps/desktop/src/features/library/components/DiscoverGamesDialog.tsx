// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

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
import { foundGamesText, offeredPaths } from "@/lib/found-games";
import { whereFrom } from "@/lib/links";
import * as api from "@/lib/tauri";
import type { DiscoveredGame, NewGames } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";
import { plural } from "@/lib/words";

const SOURCE_LABELS: Record<string, string> = {
  steam: "Steam",
  epic: "Epic Games",
  battlenet: "Battle.net",
  riot: "Riot",
  hoyoplay: "HoYoPlay",
  ubisoft: "Ubisoft Connect",
  ea: "EA app",
  rockstar: "Rockstar",
  xbox: "Xbox",
  amazon: "Amazon Games",
  itch: "itch.io",
  gog: "GOG",
  heroic: "Heroic",
  lutris: "Lutris",
  folder_scan: "Folder",
};

/** Lowercase path with forward slashes and no trailing one, for comparing paths. */
function pathKey(path: string): string {
  return path.replaceAll("\\", "/").replace(/\/+$/, "").toLowerCase();
}

/** Games found at start, by title. All of them come from launchers, so all start selected. */
function foundResults(found: NewGames | null | undefined): DiscoveredGame[] {
  return [...(found?.games ?? [])].sort((a, b) => a.title.localeCompare(b.title));
}

export function DiscoverGamesDialog({
  open,
  onOpenChange,
  onImported,
  found,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onImported: () => void;
  /** Games the look at start found. The dialog opens with them instead of a scan. */
  found?: NewGames | null;
}) {
  const [scanning, setScanning] = useState(false);
  const [importing, setImporting] = useState(false);
  const [results, setResults] = useState<DiscoveredGame[]>(() => foundResults(found));
  const [selected, setSelected] = useState<Set<string>>(
    () => new Set(foundResults(found).map((game) => game.executable_path)),
  );
  const [scanned, setScanned] = useState(Boolean(found));
  // The found games while the dialog shows them, until a full scan. Kept
  // while the dialog closes, so its words stay.
  const [shownFound, setShownFound] = useState(found ?? null);
  const showingFound = shownFound !== null;
  // Games marked as another install of a library game that the player keeps as games of their own.
  const [apart, setApart] = useState<Set<string>>(new Set());
  const [error, setError] = useState<string | null>(null);

  // Start over every time the dialog opens, with the found games if there are any.
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) {
      const initial = foundResults(found);
      setResults(initial);
      setSelected(new Set(initial.map((game) => game.executable_path)));
      setScanned(Boolean(found));
      setShownFound(found ?? null);
      setApart(new Set());
      setError(null);
      setScanning(false);
      setImporting(false);
    }
  }

  async function handleScan() {
    try {
      setScanning(true);
      setShownFound(null);
      setError(null);
      setResults([]);
      setSelected(new Set());
      setApart(new Set());

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

      // Launcher entries win over folder matches for the same executable, and a
      // folder match inside a launcher game's install folder is the same game.
      const launcherFolders = [...steamGames, ...launcherGames]
        .map((game) => game.install_folder)
        .filter((folder): folder is string => Boolean(folder))
        .map(pathKey);
      const byPath = new Map<string, DiscoveredGame>();
      for (const game of [...steamGames, ...launcherGames, ...folderGames]) {
        const insideLauncherFolder =
          game.source === "folder_scan" &&
          launcherFolders.some((folder) => pathKey(game.executable_path).startsWith(`${folder}/`));
        if (!insideLauncherFolder && !byPath.has(game.executable_path)) byPath.set(game.executable_path, game);
      }
      const merged = [...byPath.values()].sort((a, b) =>
        a.already_added !== b.already_added ? (a.already_added ? 1 : -1) : a.title.localeCompare(b.title),
      );
      setResults(merged);
      // What the player saw here never comes back as found at start.
      api.rememberOfferedGames(offeredPaths(merged)).catch((rememberError) => {
        console.warn("offered games not remembered", rememberError);
      });

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

  async function ignore(game: DiscoveredGame) {
    try {
      setError(null);
      await api.ignoreProgram(game.executable_path, game.title);
      setResults((previous) => previous.filter((entry) => entry.executable_path !== game.executable_path));
      setSelected((previous) => {
        const next = new Set(previous);
        next.delete(game.executable_path);
        return next;
      });
    } catch (ignoreError) {
      setError(describeError(ignoreError));
    }
  }

  function toggleApart(path: string) {
    setApart((previous) => {
      const next = new Set(previous);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
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
      await api.importDiscoveredGames(
        toImport.map((game) => (apart.has(game.executable_path) ? { ...game, same_game: null } : game)),
      );
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
  const foundText = shownFound ? foundGamesText(shownFound) : null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>{foundText?.title ?? "Discover games"}</DialogTitle>
          <DialogDescription>
            {foundText?.description ??
              "Reads what Steam, Xbox, Epic, GOG, EA, Ubisoft, Battle.net, Riot, Rockstar, Amazon, itch.io and other launchers have installed, and looks through the usual install folders on every drive."}
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
                {showingFound ? plural(results.length, "new game") : `${plural(results.length, "game")} found`}
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

            <ul className="max-h-[min(20rem,calc(100dvh-21rem))] overflow-y-auto border-y border-rule">
              {results.map((game) => {
                const isSelected = selected.has(game.executable_path);
                const added = game.already_added;
                const same = !added ? game.same_game : null;
                return (
                  <li
                    key={game.executable_path}
                    className="group flex flex-wrap items-center border-b border-rule transition-colors last:border-b-0 hover:bg-raised/60"
                  >
                    <button
                      type="button"
                      aria-pressed={added || isSelected}
                      disabled={added}
                      onClick={() => toggle(game.executable_path)}
                      className="flex min-w-0 flex-1 items-center gap-3 px-1 py-2.5 text-left focus-visible:bg-raised focus-visible:outline-none disabled:opacity-50"
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
                    {!added && (
                      <button
                        type="button"
                        onClick={() => void ignore(game)}
                        title="Not a game. Never suggest it again and never track it."
                        className="shrink-0 rounded-full px-2.5 py-1 text-xs text-faint opacity-0 transition-opacity group-hover:opacity-100 hover:text-text focus-visible:opacity-100 focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none"
                      >
                        Ignore
                      </button>
                    )}
                    {same && (
                      <label className="-mt-1.5 flex basis-full cursor-pointer items-center gap-2 pb-2.5 pl-9 text-[13px] text-soft">
                        <input
                          type="checkbox"
                          checked={!apart.has(game.executable_path)}
                          onChange={() => toggleApart(game.executable_path)}
                          className="accent-violet"
                        />
                        <span className="min-w-0 truncate">
                          Same game as {same.title} {whereFrom(same.launcher_source)}
                        </span>
                      </label>
                    )}
                  </li>
                );
              })}
            </ul>
            {showingFound && (
              <p className="pt-3 text-xs text-faint">
                Games you leave out are not offered again. Discover in the library still lists everything.
              </p>
            )}
          </div>
        )}

        <DialogFooter>
          <DialogClose render={<Button variant="ghost" />}>{showingFound ? "Not now" : "Cancel"}</DialogClose>
          {scanned && (
            <Button variant="outline" onClick={handleScan} disabled={scanning}>
              {scanning ? <Loader2 className="size-4 animate-spin" /> : <Search className="size-4" />}
              {showingFound ? "Show everything" : "Scan again"}
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
