// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, Navigate, useParams } from "react-router";
import { ChevronLeft, ChevronRight, Share } from "lucide-react";
import { Notice, StepButton } from "@/components/layout/Page";
import { Button } from "@/components/ui/button";
import { useLibrary } from "@/features/library/library-context";
import { yearStats } from "@/lib/stats";
import * as api from "@/lib/tauri";
import { formatDayAndMonth } from "@/lib/time";
import type { YearPlay } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";
import { amountWords, capitalize, countWords } from "@/lib/words";
import { chapterTitle, yearStory, type YearChapter } from "@/lib/year-story";
import { ShareYearDialog } from "./ShareYearDialog";
import { StoryCard } from "./YearChapters";

/** Year in games: a year told in chapters, one card at a time, made on this PC. */
export function YearInGamesPage() {
  const { year } = useParams();
  // Another year starts over at its first chapter.
  return <YearStory key={year ?? ""} />;
}

function YearStory() {
  const { year: param } = useParams();
  const thisYear = new Date().getFullYear();
  const year = param === undefined ? thisYear : Number(param);
  const valid = Number.isInteger(year) && year >= 1 && year <= thisYear;
  const { sessions, summaries, covers, statusChanges, loaded } = useLibrary();
  const [play, setPlay] = useState<{ year: number; play: YearPlay } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [position, setPosition] = useState(0);
  // Chapters already shown, whose numbers sit still when they come back.
  const [seen, setSeen] = useState<ReadonlySet<YearChapter>>(() => new Set());
  const [sharing, setSharing] = useState(false);

  useEffect(() => {
    if (!valid) return;
    let cancelled = false;
    api.getYearPlay(year).then(
      (next) => {
        if (cancelled) return;
        setPlay({ year, play: next });
        setError(null);
      },
      (failure) => !cancelled && setError(describeError(failure)),
    );
    return () => {
      cancelled = true;
    };
  }, [year, valid, sessions]);

  const byGame = useMemo(() => new Map(summaries.map((summary) => [summary.game.id, summary])), [summaries]);
  const told = useMemo(() => {
    if (!play || play.year !== year) return null;
    const stats = yearStats(sessions, year, play.play);
    const story = yearStory({
      stats,
      sessions,
      launchers: play.play.launchers,
      statusChanges,
      named: (gameId) => {
        const summary = byGame.get(gameId);
        return summary !== undefined && !summary.game.is_hidden;
      },
    });
    return { stats, story };
  }, [play, year, sessions, statusChanges, byGame]);

  const chapters = useMemo(() => told?.story.chapters ?? [], [told]);
  const at = Math.min(position, Math.max(0, chapters.length - 1));
  const chapter = chapters[at];
  const go = useCallback(
    (next: number) => {
      if (next < 0 || next >= chapters.length || next === at) return;
      setSeen((current) => new Set(current).add(chapters[at]));
      setPosition(next);
    },
    [chapters, at],
  );

  useEffect(() => {
    function onKey(event: KeyboardEvent) {
      if (sharing || event.altKey || event.ctrlKey || event.metaKey) return;
      const target = event.target instanceof HTMLElement ? event.target : null;
      if (target?.closest("input, textarea, select, [contenteditable=true], [role=dialog]")) return;
      // Space on a button presses the button.
      const forward = event.key === "ArrowRight" || (event.key === " " && !target?.closest("button, a"));
      if (!forward && event.key !== "ArrowLeft") return;
      event.preventDefault();
      go(forward ? at + 1 : at - 1);
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [at, go, sharing]);

  if (!valid) return <Navigate to="/stats" replace />;
  if (!loaded || (!told && !error)) return null;

  const story = told?.story;
  const current = year === thisYear;
  const span =
    story?.firstDay && story.lastDay
      ? story.daysPlayed === 1
        ? `on ${formatDayAndMonth(story.firstDay)}`
        : `on ${countWords(story.daysPlayed, "day")}, from ${formatDayAndMonth(story.firstDay)} to ${formatDayAndMonth(story.lastDay)}`
      : null;
  // Under a minute of play tells no chapter.
  const intro =
    story && span && chapters.length > 0
      ? `${capitalize(amountWords(story.playedMs))} ${span}. Made on this PC, nothing is uploaded.`
      : error
        ? "Your year did not load."
        : current
          ? "Nothing played this year yet. Once you play, your year fills in here."
          : `Nothing played in ${year}.`;
  const cover = (gameId: string) => covers[gameId] ?? byGame.get(gameId)?.cover ?? null;

  return (
    <div className="flex min-h-full flex-col px-8 pt-4 pb-8 xl:px-14">
      <div className="grid flex-1 items-center gap-x-12 gap-y-8 lg:grid-cols-[minmax(240px,320px)_minmax(0,1fr)]">
        <div className="self-start lg:self-center">
          <Link to="/stats" className="label-small transition-colors hover:text-text">
            Year in games
          </Link>
          <h1 className="font-display mt-1.5 text-[46px] leading-[1.02]">Your {year}</h1>
          <p className="font-prose mt-3 text-[17px] leading-[1.45] text-pretty text-soft">{intro}</p>
          {story && chapters.length > 0 && (
            <>
              <ol aria-label="Chapters" className="mt-6 flex flex-col gap-px">
                {chapters.map((each, index) => (
                  <li key={each}>
                    <button
                      type="button"
                      aria-current={index === at ? "step" : undefined}
                      onClick={() => go(index)}
                      className={cn(
                        "flex h-9 w-full items-center gap-3 rounded-[7px] px-2.5 text-left text-[13.5px] transition-colors focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none",
                        index === at ? "bg-glint/8 text-text" : "text-soft hover:bg-glint/5 hover:text-text",
                      )}
                    >
                      <span className="w-5 text-[12px] text-faint tabular-nums">
                        {String(index + 1).padStart(2, "0")}
                      </span>
                      {chapterTitle(each, story)}
                    </button>
                  </li>
                ))}
              </ol>
              <Button variant="outline" className="mt-6" onClick={() => setSharing(true)}>
                <Share strokeWidth={1.8} />
                Share your year
              </Button>
            </>
          )}
          {error && (
            <Notice tone="warning" className="mt-6">
              {error}
            </Notice>
          )}
        </div>

        {told && story && chapter && (
          <div className="flex flex-col items-center">
            <div className="flex items-center gap-5">
              <StepButton label="Previous chapter" disabled={at === 0} onClick={() => go(at - 1)}>
                <ChevronLeft className="size-[18px]" strokeWidth={1.8} />
              </StepButton>
              <StoryCard
                chapter={chapter}
                position={at}
                count={chapters.length}
                onPick={go}
                story={story}
                stats={told.stats}
                game={(gameId) => byGame.get(gameId)}
                cover={cover}
                animate={!seen.has(chapter)}
              />
              <StepButton label="Next chapter" disabled={at === chapters.length - 1} onClick={() => go(at + 1)}>
                <ChevronRight className="size-[18px]" strokeWidth={1.8} />
              </StepButton>
            </div>
            <p className="mt-4 text-[12.5px] text-faint">Space or the arrow keys move on.</p>
          </div>
        )}
      </div>
      {told && <ShareYearDialog open={sharing} onOpenChange={setSharing} stats={told.stats} now={new Date()} />}
    </div>
  );
}
