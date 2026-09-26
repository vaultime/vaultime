// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import { ArrowRight } from "lucide-react";
import {
  TintedButton,
  TintedHeader,
  TintedOverline,
  TintedSentence,
  TintedTitle,
} from "@/components/layout/Tinted";
import { Cover } from "@/components/media/Cover";
import { PhraseText } from "@/components/media/PhraseText";
import type { GameSummary } from "@/features/library/library-context";
import { BRAND_TINT } from "@/lib/game-tint";
import { lastPlayedLine, libraryPlaytime } from "@/lib/sentences";
import { numberWords } from "@/lib/words";

type HeroProps =
  | { kind: "game"; summary: GameSummary; playing: boolean; weekRuntimeMs: number }
  | { kind: "welcome"; onDiscover: () => void; onAdd: () => void }
  | { kind: "unplayed"; gameCount: number; onDiscover: () => void; onAdd: () => void };

/** The top of the library: the game you play right now or last, in its own colors. */
export function LibraryHero(props: HeroProps) {
  if (props.kind === "welcome") {
    return (
      <TintedHeader tint={BRAND_TINT}>
        <TintedOverline tint={BRAND_TINT}>Welcome to Vaultime</TintedOverline>
        <TintedTitle tint={BRAND_TINT} text="Start your library" />
        <TintedSentence tint={BRAND_TINT}>
          Find your Steam games or add any game by hand. Vaultime keeps time for everything you
          play, on this PC.
        </TintedSentence>
        <div className="mt-7 flex flex-wrap gap-3">
          <TintedButton tint={BRAND_TINT} solid onClick={props.onDiscover}>
            Discover games
          </TintedButton>
          <TintedButton tint={BRAND_TINT} onClick={props.onAdd}>
            Add a game
          </TintedButton>
        </div>
      </TintedHeader>
    );
  }

  if (props.kind === "unplayed") {
    return (
      <TintedHeader tint={BRAND_TINT}>
        <TintedOverline tint={BRAND_TINT}>
          Watching {numberWords(props.gameCount)} game{props.gameCount === 1 ? "" : "s"}
        </TintedOverline>
        <TintedTitle tint={BRAND_TINT} text="Nothing played yet" />
        <TintedSentence tint={BRAND_TINT}>
          Start any game from your library, however you usually launch it, and the clock starts
          on its own.
        </TintedSentence>
        <div className="mt-7 flex flex-wrap gap-3">
          <TintedButton tint={BRAND_TINT} solid onClick={props.onDiscover}>
            Discover more games
          </TintedButton>
          <TintedButton tint={BRAND_TINT} onClick={props.onAdd}>
            Add a game
          </TintedButton>
        </div>
      </TintedHeader>
    );
  }

  const { summary, playing, weekRuntimeMs } = props;
  const { game, cover, tint } = summary;

  return (
    <TintedHeader tint={tint} backdrop={cover}>
      <div className="flex items-end gap-12">
        <div className="min-w-0 flex-1">
          <TintedOverline tint={tint}>
            {playing && <span className="size-2 animate-live-ring rounded-full bg-violet" />}
            {playing ? "Playing now" : lastPlayedLine(summary.lastPlayedAt)}
          </TintedOverline>
          <TintedTitle tint={tint} text={game.title} />
          <TintedSentence tint={tint}>
            <PhraseText phrase={libraryPlaytime(summary.runtimeMs, weekRuntimeMs)} />
          </TintedSentence>
          <div className="mt-7 flex flex-wrap gap-3">
            <TintedButton tint={tint} solid to={`/library/${game.id}`}>
              Open game
              <ArrowRight className="size-4" strokeWidth={1.8} />
            </TintedButton>
            <TintedButton tint={tint} to="/sessions">
              All sessions
            </TintedButton>
          </div>
        </div>
        <Cover
          title={game.title}
          src={cover}
          variant="card"
          className="hidden h-[280px] w-[210px] p-[18px] text-[32px] shadow-2xl shadow-black/40 lg:flex"
        />
      </div>
    </TintedHeader>
  );
}
