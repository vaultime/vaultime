// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { useMemo, useState, type KeyboardEvent } from "react";
import { useNavigate } from "react-router";
import { Dialog as DialogPrimitive } from "@base-ui/react/dialog";
import { Search } from "lucide-react";
import { Cover } from "@/components/media/Cover";
import { useLibrary } from "@/features/library/library-context";
import { PALETTE_GAMES_IDLE, PALETTE_GAMES_SEARCHING } from "@/lib/constants";
import { formatHoursShort } from "@/lib/time";
import { cn } from "@/lib/utils";

interface Command {
  id: string;
  label: string;
  hint: string;
  kind: "Game" | "Page" | "Action";
  to: string;
  cover?: { title: string; src: string | null };
}

const FIXED: Command[] = [
  { id: "page-library", label: "Library", hint: "All your games", kind: "Page", to: "/library" },
  { id: "page-journal", label: "Journal", hint: "Your play, week by week", kind: "Page", to: "/journal" },
  { id: "page-cloud", label: "Cloud", hint: "Encrypted backups on the server", kind: "Page", to: "/cloud" },
  { id: "page-settings", label: "Settings", hint: "Tracking, backups and more", kind: "Page", to: "/settings" },
  { id: "action-add", label: "Add a game", hint: "Pick an executable by hand", kind: "Action", to: "/library?add=1" },
  { id: "action-discover", label: "Discover games", hint: "Find Steam and launcher games", kind: "Action", to: "/library?discover=1" },
  { id: "action-backup", label: "Export a local backup", hint: "Settings, local backups", kind: "Action", to: "/settings" },
  { id: "action-idle", label: "Idle threshold", hint: "Settings, tracking", kind: "Action", to: "/settings" },
];

export function CommandPalette({ open, onOpenChange }: { open: boolean; onOpenChange: (open: boolean) => void }) {
  const navigate = useNavigate();
  const { summaries } = useLibrary();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);

  const results = useMemo(() => {
    const needle = query.trim().toLowerCase();
    const matches = (text: string) => !needle || text.toLowerCase().includes(needle);
    const games: Command[] = summaries
      .filter(({ game }) => matches(game.title))
      .slice(0, needle ? PALETTE_GAMES_SEARCHING : PALETTE_GAMES_IDLE)
      .map(({ game, cover, runtimeMs }) => ({
        id: `game-${game.id}`,
        label: game.title,
        hint: runtimeMs > 0 ? `${formatHoursShort(runtimeMs)} played` : "Not played yet",
        kind: "Game",
        to: `/library/${game.id}`,
        cover: { title: game.title, src: cover },
      }));
    return [...games, ...FIXED.filter((command) => matches(command.label) || matches(command.hint))];
  }, [query, summaries]);

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
    navigate(command.to);
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
          className="fixed top-[18vh] left-1/2 z-50 flex w-[min(640px,calc(100%-2rem))] -translate-x-1/2 flex-col overflow-hidden rounded-2xl border border-hairline bg-surface shadow-2xl shadow-black/50 outline-none data-open:animate-in data-open:fade-in-0 data-open:zoom-in-95 data-closed:animate-out data-closed:fade-out-0"
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
