// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useMemo, useState, type ReactNode } from "react";
import { AddSessionDialog } from "@/features/game-details/AddSessionDialog";
import { useLibrary } from "@/features/library/library-context";
import { countedBySteam } from "@/lib/links";
import * as api from "@/lib/tauri";
import type { AwayTimer, GameLink } from "@/lib/types";
import { logFailure } from "@/lib/utils";
import { AwayTimerContext } from "./away-timer-context";

/** The timer under the game it counts as, for a game linked since it started. */
function countedAs(timer: AwayTimer | null, links: GameLink[]): AwayTimer | null {
  const target = timer && links.find((link) => link.game_id === timer.game_id)?.linked_game_id;
  return timer && target ? { ...timer, game_id: target } : timer;
}

function sameTimer(a: AwayTimer | null, b: AwayTimer | null): boolean {
  return a?.game_id === b?.game_id && a?.started_at === b?.started_at;
}

/** The timer for play away from this PC, and the dialog that saves its play. */
export function AwayTimerProvider({ children }: { children: ReactNode }) {
  const { games, summaries, links, thisDeviceId, refresh, loaded } = useLibrary();
  const [timer, setTimer] = useState<AwayTimer | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    let cancelled = false;
    api
      .getAwayTimer()
      .then((current) => {
        if (!cancelled) setTimer(current);
      })
      .catch(logFailure("reading the timer for play away"));
    return () => {
      cancelled = true;
    };
  }, []);

  const shown = useMemo(() => countedAs(timer, links), [timer, links]);
  const game = shown ? games.find((each) => each.id === shown.game_id) : undefined;
  // A timer whose game was deleted since: asked again, the core drops it. The
  // same timer back changes nothing, so a game the core keeps but no page
  // shows is asked about once.
  useEffect(() => {
    if (!loaded || !timer || game) return;
    let cancelled = false;
    api
      .getAwayTimer()
      .then((current) => {
        if (!cancelled) setTimer((held) => (sameTimer(held, current) ? held : current));
      })
      .catch(logFailure("reading the timer for play away"));
    return () => {
      cancelled = true;
    };
  }, [loaded, timer, game]);

  const earlier = summaries.find((summary) => summary.game.id === shown?.game_id)?.earlier;
  const finish = async () => {
    await api.clearAwayTimer();
    setTimer(null);
  };

  return (
    <AwayTimerContext.Provider
      value={{
        timer: shown,
        start: async (gameId) => setTimer(await api.startAwayTimer(gameId)),
        stop: () => setSaving(true),
      }}
    >
      {children}
      {saving && timer && game && (
        <AddSessionDialog
          gameId={game.id}
          gameTitle={game.title}
          steamGame={countedBySteam(game, earlier?.source, links, thisDeviceId)}
          timedFrom={timer.started_at}
          open
          onOpenChange={setSaving}
          onAdded={() => {
            finish().catch(logFailure("stopping the timer for play away"));
            void refresh();
          }}
          onDiscard={() => {
            setSaving(false);
            finish().catch(logFailure("stopping the timer for play away"));
          }}
        />
      )}
    </AwayTimerContext.Provider>
  );
}
