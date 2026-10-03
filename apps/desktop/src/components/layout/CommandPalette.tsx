// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useMemo, useState, type KeyboardEvent } from "react";
import { matchPath, useLocation, useNavigate } from "react-router";
import { Dialog as DialogPrimitive } from "@base-ui/react/dialog";
import { Search } from "lucide-react";
import { Cover } from "@/components/media/Cover";
import { useLibrary } from "@/features/library/library-context";
import { useUpdates } from "@/features/updates/update-context";
import { PALETTE_GAMES_IDLE, PALETTE_GAMES_SEARCHING } from "@/lib/constants";
import { GAME_STATUS_LABELS, GAME_STATUSES } from "@/lib/game-status";
import { matchesSearch } from "@/lib/search";
import { stepsAside } from "@/lib/steps-aside";
import * as api from "@/lib/tauri";
import { formatHoursShort } from "@/lib/time";
import { cn } from "@/lib/utils";

interface Command {
  id: string;
  label: string;
  hint: string;
  kind: "Game" | "Page" | "Action" | "Setting";
  /** Where the command goes. */
  to?: string;
  /** What the command does, in place of going somewhere. */
  run?: () => Promise<unknown>;
  cover?: { title: string; src: string | null };
}

const FIXED: Command[] = [
  { id: "page-library", label: "Library", hint: "All your games", kind: "Page", to: "/library" },
  { id: "page-journal", label: "Journal", hint: "Your play, week by week", kind: "Page", to: "/journal" },
  { id: "page-stats", label: "Stats", hint: "Your year in numbers", kind: "Page", to: "/stats" },
  { id: "page-cloud", label: "Cloud", hint: "Encrypted backups on the server", kind: "Page", to: "/cloud" },
  { id: "page-settings", label: "Settings", hint: "Tracking, backups and more", kind: "Page", to: "/settings" },
  { id: "action-add", label: "Add a game", hint: "Pick its program by hand", kind: "Action", to: "/library?add=1" },
  { id: "action-discover", label: "Discover games", hint: "Scan Steam and the folders of other launchers", kind: "Action", to: "/library?discover=1" },
  { id: "action-backup", label: "Save a backup", hint: "Pick a folder and save everything now", kind: "Action", to: "/settings?do=backup#local-backups" },
  { id: "action-export-csv", label: "Export as CSV", hint: "Every session for a spreadsheet", kind: "Action", to: "/settings?do=export-csv#export" },
  { id: "action-export-json", label: "Export as JSON", hint: "Sessions, games, statuses and earlier playtime", kind: "Action", to: "/settings?do=export-json#export" },
  { id: "setting-idle", label: "Idle time", hint: "When time stops counting as active", kind: "Setting", to: "/settings#tracking" },
  { id: "setting-this-pc", label: "This PC", hint: "Its name and the ledger of its sessions", kind: "Setting", to: "/settings#this-pc" },
  { id: "setting-other-pcs", label: "Other PCs", hint: "Merge the sessions of another PC from its backup", kind: "Setting", to: "/settings#other-pcs" },
  { id: "setting-backups", label: "Local backups", hint: "Save, restore and where daily backups go", kind: "Setting", to: "/settings#local-backups" },
  { id: "setting-appearance", label: "Appearance", hint: "Light or dark, ground, accent and background", kind: "Setting", to: "/settings#appearance" },
  { id: "setting-window", label: "Window size", hint: "Compact to Extra large, or free", kind: "Setting", to: "/settings#window" },
  { id: "setting-ignored", label: "Ignored programs", hint: "Programs that are no game", kind: "Setting", to: "/settings#ignored-programs" },
  { id: "setting-earlier", label: "Earlier playtime", hint: "Steam's count from before Vaultime", kind: "Setting", to: "/settings#earlier-playtime" },
  { id: "setting-about", label: "About", hint: "Version, license and updates", kind: "Setting", to: "/settings#about" },
];

export function CommandPalette({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const navigate = useNavigate();
  const location = useLocation();
  const { visible: summaries, summaries: everyGame, setStatus, refresh } = useLibrary();
  const updates = useUpdates();
  // On a game's page, things to do with that game come first.
  const pageGameId = matchPath("/library/:gameId", location.pathname)?.params.gameId;
  const pageGame = everyGame.find((summary) => summary.game.id === pageGameId);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);

  const results = useMemo(() => {
    const matches = (text: string) => matchesSearch(text, query);
    const games: Command[] = summaries
      .filter(({ game }) => matches(game.title))
      .slice(0, query.trim() ? PALETTE_GAMES_SEARCHING : PALETTE_GAMES_IDLE)
      .map(({ game, cover, totalMs }) => ({
        id: `game-${game.id}`,
        label: game.title,
        hint: totalMs > 0 ? `${formatHoursShort(totalMs)} played` : "Not played yet",
        kind: "Game",
        to: `/library/${game.id}`,
        cover: { title: game.title, src: cover },
      }));
    const fixed: Command[] = [
      ...FIXED,
      {
        id: "action-updates",
        label: "Check for updates",
        hint: "Look for a new version of Vaultime now",
        kind: "Action",
        to: "/settings#about",
        run: updates.check,
      },
    ];
    const forGame: Command[] = [];
    if (pageGame) {
      const { game, status } = pageGame;
      for (const next of GAME_STATUSES.filter((entry) => entry !== status)) {
        forGame.push({
          id: `status-${next}`,
          label: `Mark ${game.title} as ${GAME_STATUS_LABELS[next].toLowerCase()}`,
          hint: "Its status in the library and the journal",
          kind: "Action",
          run: () => setStatus(game.id, next),
        });
      }
      if (status) {
        forGame.push({
          id: "status-none",
          label: `Clear the status of ${game.title}`,
          hint: "No status in the library",
          kind: "Action",
          run: () => setStatus(game.id, "none"),
        });
      }
      forGame.push({
        id: "hide",
        label: game.is_hidden ? `Show ${game.title} in the library` : `Hide ${game.title}`,
        hint: "Still tracked either way",
        kind: "Action",
        run: () => api.updateGame(game.id, { is_hidden: !game.is_hidden }).then(refresh),
      });
      const stepping = stepsAside(game);
      forGame.push({
        id: "steps-aside",
        label: stepping ? `Count ${game.title} always` : `Count ${game.title} only when no other game runs`,
        hint: stepping ? "Its time beside other games counts again" : "For launchers and game clients",
        kind: "Action",
        run: () => api.setGameStepsAside(game.id, !stepping).then(refresh),
      });
    }
    return [
      ...forGame.filter((command) => matches(command.label)),
      ...games,
      ...fixed.filter((command) => matches(command.label) || matches(command.hint)),
    ];
  }, [query, summaries, pageGame, setStatus, refresh, updates.check]);

  const active = Math.min(selected, Math.max(results.length - 1, 0));

  function close(next: boolean) {
    onOpenChange(next);
    if (!next) {
      setQuery("");
      setSelected(0);
    }
  }

  function run(command: Command | undefined) {
    if (!command) return;
    close(false);
    if (command.run) void command.run().catch(() => {});
    if (command.to) navigate(command.to);
  }

  function onKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelected((active + 1) % Math.max(results.length, 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelected((active - 1 + results.length) % Math.max(results.length, 1));
    } else if (event.key === "Enter") {
      event.preventDefault();
      run(results[active]);
    }
  }

  return (
    <DialogPrimitive.Root open={open} onOpenChange={close}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Backdrop className="fixed inset-0 z-50 bg-ink/70 backdrop-blur-[3px] data-open:animate-in data-open:fade-in-0 data-closed:animate-out data-closed:fade-out-0" />
        <DialogPrimitive.Popup
          aria-label="Command palette"
          className="fixed top-[18vh] left-1/2 z-50 flex w-[min(640px,calc(100%-2rem))] -translate-x-1/2 flex-col overflow-hidden rounded-2xl border border-hairline bg-surface shadow-2xl shadow-scrim/50 outline-none data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0"
        >
          <label className="flex h-14 items-center gap-3 border-b border-rule px-5 text-faint">
            <Search className="size-[18px] shrink-0" strokeWidth={1.6} />
            <input
              autoFocus
              value={query}
              onChange={(event) => {
                setQuery(event.target.value);
                setSelected(0);
              }}
              onKeyDown={onKeyDown}
              placeholder="Search games, pages and actions"
              aria-label="Search games, pages and actions"
              role="combobox"
              aria-expanded="true"
              aria-controls="command-results"
              aria-activedescendant={results[active] ? `command-${results[active].id}` : undefined}
              className="min-w-0 flex-1 bg-transparent text-base text-text outline-none placeholder:text-faint"
            />
            <kbd className="rounded-[5px] border border-hairline px-1.5 py-0.5 font-mono text-[11px]">Esc</kbd>
          </label>

          <ul id="command-results" role="listbox" aria-label="Results" className="max-h-[360px] overflow-y-auto p-1.5">
            {results.length === 0 && <li className="px-4 py-6 text-center text-sm text-faint">Nothing matches "{query}".</li>}
            {results.map((command, index) => (
              <li
                key={command.id}
                id={`command-${command.id}`}
                role="option"
                aria-selected={index === active}
                onMouseEnter={() => setSelected(index)}
                onClick={() => run(command)}
                className={cn(
                  "flex min-h-[52px] cursor-pointer items-center gap-3 rounded-[10px] px-3.5 py-2",
                  index === active && "bg-raised",
                )}
              >
                {command.cover && (
                  <Cover title={command.cover.title} src={command.cover.src} variant="tile" className="size-8 text-base" />
                )}
                <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                  <span className="truncate text-sm text-text">{command.label}</span>
                  <span className="truncate text-xs text-faint">{command.hint}</span>
                </span>
                <span className="font-mono text-[11px] tracking-wide text-faint uppercase">{command.kind}</span>
              </li>
            ))}
          </ul>
        </DialogPrimitive.Popup>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}
