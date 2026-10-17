// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { Pencil, Unlink } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { EditGameDialog, type EditableGame } from "@/features/library/components/EditGameDialog";
import { GameLinkPicker } from "@/features/library/components/GameLinkPicker";
import { useLibrary } from "@/features/library/library-context";
import { pcName } from "@/lib/devices";
import { linkTargets, linkedTo, whereFrom } from "@/lib/links";
import * as api from "@/lib/tauri";
import { formatHoursMinutes } from "@/lib/time";
import type { Game, GameLink, Session, StepsAsideHint } from "@/lib/types";
import { describeError } from "@/lib/utils";

/** Why the last change in a row failed. */
function RowError({ error }: { error: string | null }) {
  return error ? <p className="mt-2 text-[12px] text-amber">{error}</p> : null;
}

/** Links a game to another, or with null back to itself, then reloads the library. */
function useLinkGame() {
  const { refresh } = useLibrary();
  const [error, setError] = useState<string | null>(null);
  async function link(gameId: string, linkedGameId: string | null): Promise<boolean> {
    setError(null);
    try {
      await api.linkGame(gameId, linkedGameId);
      await refresh();
      return true;
    } catch (linkError) {
      setError(describeError(linkError));
      return false;
    }
  }
  return { link, error };
}

/**
 * The switch that makes a game count only while no other game runs, with a
 * hint when other games often ran beside it, like a launcher left open.
 */
export function StepsAsideRow({
  game,
  on,
  sessions,
  error,
  onChange,
}: {
  game: Game;
  /** Whether the game counts only while no other game runs. */
  on: boolean;
  /** Every session, so the hint is asked for again when one starts or ends. */
  sessions: Session[];
  error: string | null;
  onChange: (next: boolean) => void;
}) {
  // The core says when the game looks like a launcher left open.
  const [hint, setHint] = useState<{ gameId: string; hint: StepsAsideHint | null } | null>(null);
  useEffect(() => {
    let cancelled = false;
    api
      .getStepsAsideHint(game.id)
      .then((next) => {
        if (!cancelled) setHint({ gameId: game.id, hint: next });
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [game.id, on, sessions]);
  const suggest = !on && hint?.gameId === game.id ? hint.hint : null;
  return (
    <div className="mt-4 border-t border-rule pt-4">
      <div className="flex items-start justify-between gap-4">
        <label htmlFor="steps-aside" className="min-w-0">
          <span className="block text-[13px] text-text">Count only when no other game runs</span>
          <span className="mt-1 block text-[12px] leading-relaxed text-faint">
            For launchers and game clients. Its time beside another game is set aside, and switching this off brings
            that time back.
          </span>
        </label>
        <Switch id="steps-aside" checked={on} onCheckedChange={onChange} />
      </div>
      {suggest && (
        <p className="mt-2 text-[12px] leading-relaxed text-soft">
          Games started while it was open and ran beside it for {formatHoursMinutes(suggest.beside_ms)},{" "}
          {Math.round(suggest.share * 100)} % of its time. If it is a launcher, this stops that time from counting for it.
        </p>
      )}
      <RowError error={error} />
    </div>
  );
}

/** A game that came with sessions of another PC: where from, and which game of this PC it counts as. */
export function ForeignGameRows({ game }: { game: Game }) {
  const { games, devices } = useLibrary();
  const { link, error } = useLinkGame();
  const ours = games.filter((candidate) => candidate.origin_device_id === null);
  const from = pcName(devices, game.origin_device_id ?? "");

  return (
    <div>
      <p className="text-[13px] leading-relaxed text-faint">
        Came with the sessions of {from}. Not tracked on this PC, its sessions come with each merge.
      </p>
      <div className="mt-3 flex items-center justify-between gap-3">
        <span className="text-[13px] text-text">Counts as</span>
        <GameLinkPicker games={ours} value={null} onChange={(linked) => void link(game.id, linked)} />
      </div>
      <RowError error={error} />
    </div>
  );
}

/**
 * What else counts as this game: other installs here, like a copy from a
 * second launcher, each still tracked with its own program, and games of
 * other PCs. Each can be edited where it is this PC's, and unlinked.
 */
export function LinkedGameRows({ gameId }: { gameId: string }) {
  const { links, devices, thisDeviceId, refresh } = useLibrary();
  const { link, error } = useLinkGame();
  const [editing, setEditing] = useState<EditableGame | null>(null);
  const { installs, elsewhere } = linkedTo(gameId, links, thisDeviceId);
  if (installs.length === 0 && elsewhere.length === 0) return null;

  const unlinkButton = (entry: GameLink, name: string) => (
    <Button
      variant="ghost"
      size="icon-sm"
      className="text-faint"
      aria-label={`Unlink ${name}`}
      title="Unlink, so it counts as its own game again"
      onClick={() => void link(entry.game_id, null)}
    >
      <Unlink className="size-3.5" />
    </Button>
  );

  return (
    <div className="mt-4 border-t border-rule pt-4">
      <p className="text-[13px] text-text">Also counts</p>
      <ul className="mt-1">
        {installs.map((install) => {
          const name = `${install.title} ${whereFrom(install.launcher_source)}`;
          return (
            <li key={install.game_id} className="flex items-start gap-2 border-b border-rule py-2 last:border-b-0">
              <div className="min-w-0 flex-1">
                <p className="text-[13px] text-soft">{name}</p>
                <p className="mt-0.5 font-mono text-xs leading-relaxed break-all text-faint">
                  {install.executable_path ?? "No program set, so it is not tracked."}
                </p>
              </div>
              <Button
                variant="ghost"
                size="icon-sm"
                className="text-faint"
                aria-label={`Edit ${name}`}
                title="Edit this install"
                onClick={() =>
                  setEditing({
                    id: install.game_id,
                    title: install.title,
                    executable_path: install.executable_path,
                    install_folder: install.install_folder,
                  })
                }
              >
                <Pencil className="size-3.5" />
              </Button>
              {unlinkButton(install, name)}
            </li>
          );
        })}
        {elsewhere.map((other) => {
          const pc = pcName(devices, other.origin_device_id ?? "");
          return (
            <li key={other.game_id} className="flex items-start gap-2 border-b border-rule py-2 last:border-b-0">
              <p className="min-w-0 flex-1 text-[13px] text-soft">
                Played on {pc} as “{other.title}”{other.launcher_source ? ` ${whereFrom(other.launcher_source)}` : ""}
              </p>
              {unlinkButton(other, `${other.title} on ${pc}`)}
            </li>
          );
        })}
      </ul>
      <p className="mt-2 text-[12px] leading-relaxed text-faint">
        Each install keeps its own sessions and stays tracked. Unlinking brings it back as its own game with its own
        history.
      </p>
      <RowError error={error} />
      <EditGameDialog game={editing} onClose={() => setEditing(null)} onSaved={refresh} />
    </div>
  );
}

/** Makes this game count as another game of the library, like a second install of it. */
export function SameGameRow({ game, onLinked }: { game: Game; onLinked: (targetId: string) => void }) {
  const { games } = useLibrary();
  const { link, error } = useLinkGame();
  const targets = linkTargets(game, games);
  if (targets.length === 0) return null;

  async function linkTo(targetId: string | null) {
    if (targetId && (await link(game.id, targetId))) onLinked(targetId);
  }

  return (
    <div className="mt-4 border-t border-rule pt-4">
      <div className="flex items-center justify-between gap-3">
        <span className="text-[13px] text-text">Counts as</span>
        <GameLinkPicker games={targets} value={null} onChange={(targetId) => void linkTo(targetId)} />
      </div>
      <p className="mt-2 text-[12px] leading-relaxed text-faint">
        For the same game installed twice, like through Steam and GOG. Both stay tracked and show as one game with
        one history.
      </p>
      <RowError error={error} />
    </div>
  );
}

