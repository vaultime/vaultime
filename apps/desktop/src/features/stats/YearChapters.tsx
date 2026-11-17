// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState, type ReactNode } from "react";
import { LogoMark } from "@/components/brand/Logo";
import { Figures } from "@/components/layout/StatTiles";
import { Cover } from "@/components/media/Cover";
import type { GameSummary } from "@/features/library/library-context";
import { YEAR_STORY_COUNT_UP_MS } from "@/lib/constants";
import type { YearStats } from "@/lib/stats";
import { formatClockTime, formatDate, formatHoursMinutes, formatHoursShort } from "@/lib/time";
import { cn } from "@/lib/utils";
import { chapterTitle, type YearChapter, type YearStory } from "@/lib/year-story";
import { countWords, plural, shareWords } from "@/lib/words";

interface ChapterProps {
  story: YearStory;
  stats: YearStats;
  game: (gameId: string) => GameSummary | undefined;
  cover: (gameId: string) => string | null;
  /** Counts the big number up, the first time the chapter shows. */
  animate: boolean;
}

/** The game a chapter is about, whose art fills the card. */
function subjectOf(chapter: YearChapter, story: YearStory): { gameId: string; sharp: boolean } | null {
  const top = story.game?.gameId;
  switch (chapter) {
    case "game":
      return top ? { gameId: top, sharp: true } : null;
    case "longest":
      return story.longest ? { gameId: story.longest.gameId, sharp: true } : null;
    case "week":
      return story.week?.games[0] ? { gameId: story.week.games[0].gameId, sharp: true } : null;
    case "finished":
      return story.finished[0] ? { gameId: story.finished[0].gameId, sharp: false } : null;
    case "rhythm":
    case "launchers":
      return null;
    default:
      return top ? { gameId: top, sharp: false } : null;
  }
}

/** One chapter on a card shaped like a story, with a step for each chapter on top. */
export function StoryCard({
  chapter,
  position,
  count,
  onPick,
  ...props
}: ChapterProps & {
  chapter: YearChapter;
  position: number;
  count: number;
  onPick: (position: number) => void;
}) {
  const subject = subjectOf(chapter, props.story);
  const art = subject ? props.cover(subject.gameId) : null;
  return (
    <article
      aria-label={`${chapterTitle(chapter, props.story)}, chapter ${position + 1} of ${count}`}
      className="relative isolate flex aspect-[9/16] h-[clamp(380px,calc(100dvh-250px),640px)] shrink-0 flex-col overflow-hidden rounded-[18px] bg-surface shadow-2xl ring-1 shadow-scrim/40 ring-glint/8"
    >
      {art ? (
        <img
          key={art}
          src={art}
          alt=""
          aria-hidden="true"
          className={cn(
            "absolute inset-0 -z-10 size-full object-cover duration-700 animate-in fade-in motion-reduce:animate-none",
            !subject?.sharp && "scale-125 opacity-45 blur-2xl",
          )}
        />
      ) : (
        <div
          aria-hidden="true"
          className="absolute inset-0 -z-10"
          style={{
            background:
              "radial-gradient(90% 60% at 85% 10%, color-mix(in oklab, var(--violet) 30%, transparent), transparent 70%)",
          }}
        />
      )}
      {/* Art shows at the top and fades into the card where the words sit. */}
      <div
        aria-hidden="true"
        className="absolute inset-0 -z-10"
        style={{
          background:
            "linear-gradient(to bottom, color-mix(in oklab, var(--surface) 35%, transparent) 0%, transparent 22%, color-mix(in oklab, var(--surface) 80%, transparent) 50%, var(--surface) 74%)",
        }}
      />
      <div className="flex gap-1 px-3.5 pt-3">
        {Array.from({ length: count }, (_, step) => (
          <button
            key={step}
            type="button"
            aria-label={`Chapter ${step + 1}`}
            aria-current={step === position ? "step" : undefined}
            onClick={() => onPick(step)}
            className="group flex h-3 flex-1 items-center focus-visible:outline-none"
          >
            <span
              className={cn(
                "h-[3px] w-full rounded-full transition-colors group-focus-visible:ring-2 group-focus-visible:ring-violet/60",
                step <= position ? "bg-text/85" : "bg-glint/20 group-hover:bg-glint/35",
              )}
            />
          </button>
        ))}
      </div>
      <div className="flex items-center justify-between px-5 pt-2.5 text-[12.5px] text-soft">
        <span className="flex items-center gap-1.5 text-text">
          <LogoMark className="size-4" />
          <span className="font-wordmark text-[14px] leading-none">Vaultime</span>
        </span>
        <span className="tabular-nums">{props.story.year}</span>
      </div>
      <div
        key={chapter}
        className="mt-auto px-6 pb-7 duration-500 animate-in fade-in slide-in-from-bottom-2 motion-reduce:animate-none"
      >
        <Chapter chapter={chapter} {...props} />
      </div>
    </article>
  );
}

function Chapter({ chapter, ...props }: ChapterProps & { chapter: YearChapter }) {
  switch (chapter) {
    case "numbers":
      return <NumbersChapter {...props} />;
    case "game":
      return <GameChapter {...props} />;
    case "top":
      return <TopChapter {...props} />;
    case "longest":
      return <LongestChapter {...props} />;
    default:
      return null;
  }
}

function NumbersChapter({ story, animate }: ChapterProps) {
  return (
    <>
      <Eyebrow>Your year in numbers</Eyebrow>
      <Big value={story.playedMs} animate={animate} />
      <p className="font-prose mt-1 text-[16px] text-soft">with a game running</p>
      <Facts
        facts={[
          ["Days played", String(story.daysPlayed)],
          ["Sessions", String(story.sessionsCount)],
          ["Games", String(story.gamesCount)],
          ["Best streak", plural(story.longestStreakDays, "day")],
        ]}
        columns={2}
      />
    </>
  );
}

function GameChapter({ story, game, animate }: ChapterProps) {
  const top = story.game;
  const summary = top ? game(top.gameId) : undefined;
  if (!top || !summary) return null;
  return (
    <>
      <Eyebrow color={summary.tint.ink}>Your game of the year</Eyebrow>
      <Title>{summary.game.title}</Title>
      <Big value={top.runtimeMs} animate={animate} />
      <p className="font-prose mt-1 text-[16px] leading-snug text-pretty text-soft">
        on {countWords(top.days, "day")}, {shareWords(top.share)} of your hours
      </p>
      <Facts
        facts={[
          ["Longest", formatHoursMinutes(top.longestSessionMs)],
          ["Active", `${Math.round((top.activeMs / top.runtimeMs) * 100)} %`],
          ["Best month", formatDate(new Date(story.year, top.bestMonth, 1), { month: "long" })],
        ]}
        columns={3}
      />
    </>
  );
}

function TopChapter({ story, game, cover }: ChapterProps) {
  const most = story.top[0]?.runtimeMs ?? 1;
  return (
    <>
      <Eyebrow>{chapterTitle("top", story)}</Eyebrow>
      <ol className="mt-4 flex flex-col gap-3">
        {story.top.map((entry, rank) => {
          const summary = game(entry.gameId);
          if (!summary) return null;
          return (
            <li key={entry.gameId} className="flex items-center gap-3">
              <span className="w-3 shrink-0 text-[12px] text-faint tabular-nums">{rank + 1}</span>
              <Cover
                title={summary.game.title}
                src={cover(entry.gameId)}
                variant="tile"
                className="h-[42px] w-[29px] rounded-[4px] text-[8px]"
              />
              <div className="min-w-0 flex-1">
                <div className="flex items-baseline justify-between gap-2">
                  <span className="truncate text-[14px] font-medium">{summary.game.title}</span>
                  <span className="shrink-0 text-[13px] text-soft tabular-nums">
                    {formatHoursShort(entry.runtimeMs)}
                  </span>
                </div>
                <div className="mt-1.5 h-[3px] rounded-full bg-glint/10">
                  <div
                    className="h-full rounded-full"
                    style={{ width: `${(entry.runtimeMs / most) * 100}%`, background: summary.tint.ink }}
                  />
                </div>
              </div>
            </li>
          );
        })}
      </ol>
    </>
  );
}

function LongestChapter({ story, game, animate }: ChapterProps) {
  const longest = story.longest;
  const summary = longest ? game(longest.gameId) : undefined;
  if (!longest || !summary) return null;
  return (
    <>
      <Eyebrow color={summary.tint.ink}>{chapterTitle("longest", story)}</Eyebrow>
      <Title>{summary.game.title}</Title>
      <Big value={longest.runtimeMs} animate={animate} format={formatHoursMinutes} />
      <p className="mt-2 text-[14px] leading-snug text-soft">
        {formatDate(longest.start, { weekday: "long", day: "numeric", month: "long" })},{" "}
        <span className="font-mono text-[13px]">{formatClockTime(longest.start)}</span> to{" "}
        {longest.end ? <span className="font-mono text-[13px]">{formatClockTime(longest.end)}</span> : "now"}
      </p>
      {longest.runtimeMs > 0 && (
        <p className="mt-1 text-[13px] text-faint">
          {Math.round((longest.activeMs / longest.runtimeMs) * 100)} % of it active
        </p>
      )}
    </>
  );
}

function Eyebrow({ color, children }: { color?: string; children: ReactNode }) {
  return (
    <p className={cn("text-[13px] font-medium", !color && "text-violet-hover")} style={color ? { color } : undefined}>
      {children}
    </p>
  );
}

function Title({ children }: { children: ReactNode }) {
  return <h2 className="font-display mt-1 text-[28px] leading-[1.05] text-balance">{children}</h2>;
}

/** The chapter's number, counted up from zero the first time it shows, the units smaller. */
function Big({
  value,
  animate,
  format = formatHoursShort,
}: {
  value: number;
  animate: boolean;
  format?: (ms: number) => string;
}) {
  const shown = useCountUp(value, animate);
  return (
    <div className="mt-1 text-[60px] leading-[1.05] font-semibold tracking-[-0.02em] tabular-nums">
      <Figures text={format(shown)} />
    </div>
  );
}

function Facts({ facts, columns }: { facts: [label: string, value: string][]; columns: 2 | 3 }) {
  return (
    <dl
      className={cn(
        "mt-5 grid gap-x-4 gap-y-3 border-t border-glint/10 pt-4",
        columns === 2 ? "grid-cols-2" : "grid-cols-3",
      )}
    >
      {facts.map(([label, value]) => (
        <div key={label} className="min-w-0">
          <dt className="truncate text-[12px] text-faint">{label}</dt>
          <dd className="mt-0.5 truncate text-[17px] font-semibold tabular-nums">
            {/^\d/.test(value) ? <Figures text={value} /> : value}
          </dd>
        </div>
      ))}
    </dl>
  );
}

/** `value`, counted up from zero once when `animate` is on, unless motion is turned down. */
function useCountUp(value: number, animate: boolean): number {
  const [still] = useState(() => !animate || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches);
  const [progress, setProgress] = useState(still ? 1 : 0);
  useEffect(() => {
    if (still) return;
    let frame = 0;
    const start = performance.now();
    const step = (time: number) => {
      const next = Math.min(1, (time - start) / YEAR_STORY_COUNT_UP_MS);
      setProgress(next);
      if (next < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [still]);
  // Eases out, so the number slows as it lands.
  return value * (1 - (1 - progress) ** 3);
}
