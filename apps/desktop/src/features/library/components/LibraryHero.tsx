// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

import type { CSSProperties, ReactNode } from "react";
import { Link } from "react-router";
import { ArrowRight } from "lucide-react";
import { Cover } from "@/components/media/Cover";
import type { GameSummary } from "@/features/library/library-context";
import { HERO_TITLE_LARGE_MAX_CHARS, HERO_TITLE_MEDIUM_MAX_CHARS, HOUR_MS, MINUTE_MS } from "@/lib/constants";
import { BRAND_TINT, type GameTint } from "@/lib/game-tint";
import { formatRelativeDay } from "@/lib/time";
import { capitalize, numberWords } from "@/lib/words";
import { cn } from "@/lib/utils";

type HeroProps =
  | { kind: "game"; summary: GameSummary; playing: boolean; weekRuntimeMs: number }
  | { kind: "welcome"; onDiscover: () => void; onAdd: () => void }
  | { kind: "unplayed"; gameCount: number; onDiscover: () => void; onAdd: () => void };

/** The top of the library: the game you play right now or last, in its own colors. */
export function LibraryHero(props: HeroProps) {
  if (props.kind === "welcome") {
    return (
      <HeroFrame tint={BRAND_TINT}>
        <Overline tint={BRAND_TINT}>Welcome to Vaultime</Overline>
        <Title tint={BRAND_TINT} text="Start your library" />
        <Sentence tint={BRAND_TINT}>
          Find your Steam games or add any game by hand. Vaultime keeps time for everything you
          play, on this PC.
        </Sentence>
        <Actions>
          <SolidButton tint={BRAND_TINT} onClick={props.onDiscover}>
            Discover games
          </SolidButton>
          <OutlineButton tint={BRAND_TINT} onClick={props.onAdd}>
            Add a game
          </OutlineButton>
        </Actions>
      </HeroFrame>
    );
  }

  if (props.kind === "unplayed") {
    return (
      <HeroFrame tint={BRAND_TINT}>
        <Overline tint={BRAND_TINT}>
          Watching {numberWords(props.gameCount)} game{props.gameCount === 1 ? "" : "s"}
        </Overline>
        <Title tint={BRAND_TINT} text="Nothing played yet" />
        <Sentence tint={BRAND_TINT}>
          Start any game from your library, however you usually launch it, and the clock starts
          on its own.
        </Sentence>
        <Actions>
          <SolidButton tint={BRAND_TINT} onClick={props.onDiscover}>
            Discover more games
          </SolidButton>
          <OutlineButton tint={BRAND_TINT} onClick={props.onAdd}>
            Add a game
          </OutlineButton>
        </Actions>
      </HeroFrame>
    );
  }

  const { summary, playing, weekRuntimeMs } = props;
  const { game, cover, tint } = summary;

  return (
    <HeroFrame tint={tint} backdrop={cover}>
      <div className="flex items-end gap-12">
        <div className="min-w-0 flex-1">
          <Overline tint={tint}>
            {playing ? (
              <span className="flex items-center gap-2.5">
                <span className="size-2 animate-live-ring rounded-full bg-violet" />
                Playing now
              </span>
            ) : (
              lastPlayedLine(summary.lastPlayedAt)
            )}
          </Overline>
          <Title tint={tint} text={game.title} />
          <Sentence tint={tint}>{playtimeSentence(summary.runtimeMs, weekRuntimeMs)}</Sentence>
          <Actions>
            <Link to={`/library/${game.id}`} className={SOLID} style={solidStyle(tint)}>
              Open game
              <ArrowRight className="size-4" strokeWidth={1.8} />
            </Link>
            <Link to="/sessions" className={OUTLINE} style={outlineStyle(tint)}>
              All sessions
            </Link>
          </Actions>
        </div>
        <Cover
          title={game.title}
          src={cover}
          variant="card"
          className="hidden h-[280px] w-[210px] p-[18px] text-[32px] shadow-2xl shadow-black/40 lg:flex"
        />
      </div>
    </HeroFrame>
  );
}

/** "Last played 3 hours ago", "Last played on Monday", "Last played in August". */
function lastPlayedLine(lastPlayedAt: string | null): string {
  if (!lastPlayedAt) return "Not played yet";
  const when = formatRelativeDay(lastPlayedAt);
  if (when !== "Yesterday" && when.endsWith("day")) return `Last played on ${when}`;
  if (when.startsWith("In ")) return `Last played in ${when.slice(3)}`;
  return `Last played ${when.charAt(0).toLowerCase()}${when.slice(1)}`;
}

/** "One hundred forty-two hours so far, eleven of them this past week." */
function playtimeSentence(runtimeMs: number, weekRuntimeMs: number): ReactNode {
  if (runtimeMs < HOUR_MS) {
    const minutes = Math.floor(runtimeMs / MINUTE_MS);
    if (minutes < 1) return "The clock has just started.";
    return `${capitalize(numberWords(minutes))} minute${minutes === 1 ? "" : "s"} so far.`;
  }

  const hours = Math.floor(runtimeMs / HOUR_MS);
  const weekHours = Math.floor(weekRuntimeMs / HOUR_MS);
  const total = `${capitalize(numberWords(hours))} hour${hours === 1 ? "" : "s"} so far`;
  if (weekHours >= hours) return `${total}, all of them this past week.`;
  if (weekHours === 0) return `${total}.`;
  return (
    <>
      {total}, <em>{numberWords(weekHours)}</em> of them this past week.
    </>
  );
}

function HeroFrame({ tint, backdrop, children }: { tint: GameTint; backdrop?: string | null; children: ReactNode }) {
  return (
    <section
      className="relative isolate overflow-hidden border-b px-8 pt-12 pb-10 xl:px-14"
      style={{
        borderColor: tint.fill,
        background: `radial-gradient(120% 150% at 90% 0%, ${tint.fill} 0%, ${tint.wash} 60%)`,
      }}
    >
      {backdrop && (
        <img
          src={backdrop}
          alt=""
          aria-hidden="true"
          className="absolute inset-0 -z-10 size-full scale-125 object-cover opacity-[0.18] blur-3xl"
        />
      )}
      {children}
    </section>
  );
}

function Overline({ tint, children }: { tint: GameTint; children: ReactNode }) {
  return (
    <div className="text-xs tracking-[0.16em] uppercase" style={{ color: tint.muted }}>
      {children}
    </div>
  );
}

function Title({ tint, text }: { tint: GameTint; text: string }) {
  // Long titles step down so they stay on two lines.
  const size =
    text.length <= HERO_TITLE_LARGE_MAX_CHARS
      ? "text-[clamp(56px,7vw,100px)]"
      : text.length <= HERO_TITLE_MEDIUM_MAX_CHARS
        ? "text-[clamp(48px,5.4vw,80px)]"
        : "text-[clamp(40px,4.2vw,60px)]";
  return (
    <h1
      className={cn("font-display mt-3.5 leading-[0.95] font-medium tracking-[-0.03em] text-balance", size)}
      style={{ color: tint.ink }}
    >
      {text}
    </h1>
  );
}

function Sentence({ tint, children }: { tint: GameTint; children: ReactNode }) {
  return (
    <p className="font-display mt-5 max-w-[600px] text-2xl leading-[1.3] text-pretty" style={{ color: tint.soft }}>
      {children}
    </p>
  );
}

function Actions({ children }: { children: ReactNode }) {
  return <div className="mt-7 flex flex-wrap gap-3">{children}</div>;
}

const SOLID =
  "inline-flex h-11 items-center gap-2 rounded-full px-5 text-sm font-semibold transition-opacity hover:opacity-90 focus-visible:ring-2 focus-visible:ring-offset-2 focus-visible:ring-offset-transparent focus-visible:outline-none";
const OUTLINE =
  "inline-flex h-11 items-center rounded-full border px-5 text-sm font-medium transition-colors hover:bg-white/5 focus-visible:ring-2 focus-visible:outline-none";

function solidStyle(tint: GameTint): CSSProperties {
  return { background: tint.ink, color: tint.wash };
}

function outlineStyle(tint: GameTint): CSSProperties {
  return { borderColor: tint.edge, color: tint.ink };
}

function SolidButton({ tint, onClick, children }: { tint: GameTint; onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" onClick={onClick} className={SOLID} style={solidStyle(tint)}>
      {children}
    </button>
  );
}

function OutlineButton({ tint, onClick, children }: { tint: GameTint; onClick: () => void; children: ReactNode }) {
  return (
    <button type="button" onClick={onClick} className={OUTLINE} style={outlineStyle(tint)}>
      {children}
    </button>
  );
}
