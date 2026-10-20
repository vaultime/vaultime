// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";
import { ChevronLeft, ChevronRight, Share } from "lucide-react";
import { ActiveIdleLegend } from "@/components/charts/PlayBars";
import { PeriodBars, type PeriodColumn } from "@/components/charts/PeriodBars";
import { WeekClock } from "@/components/charts/WeekClock";
import { YearHeatmap } from "@/components/charts/YearHeatmap";
import { Notice, PageHeader, PageSection, StepButton } from "@/components/layout/Page";
import { StatTiles } from "@/components/layout/StatTiles";
import { Cover } from "@/components/media/Cover";
import { PhraseText } from "@/components/media/PhraseText";
import { Button } from "@/components/ui/button";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useLibrary, usePlaying, type GameSummary } from "@/features/library/library-context";
import {
  SESSION_LONG_MAX_MS,
  SESSION_PLAIN_MAX_MS,
  SESSION_QUICK_MAX_MS,
  SESSION_SHORT_MAX_MS,
  STATS_IDLE_GAMES,
  STATS_IDLE_MIN_RUNTIME_MS,
  LIVE_TOTALS_REFRESH_MS,
  STATS_IDLE_MIN_SHARE,
  STATS_TOP_GAMES,
} from "@/lib/constants";
import { markColors, tintForTitle } from "@/lib/game-tint";
import { useRefreshWhile } from "@/lib/use-refresh-while";
import { countsAsPlay } from "@/lib/sessions";
import {
  allTimeStats,
  daysSoFar,
  SESSION_SHAPES,
  yearStats,
  type AllTimeStats,
  type GameYear,
  type SessionShape,
  type YearStats,
} from "@/lib/stats";
import * as api from "@/lib/tauri";
import {
  calendarDaysAgo,
  formatDate,
  formatDayAndMonth,
  formatDayRange,
  formatHoursMinutes,
  formatHoursShort,
  parseVaultimeDate,
  startOfDay,
  toDayKey,
} from "@/lib/time";
import type { YearPlay } from "@/lib/types";
import { cn, describeError } from "@/lib/utils";
import { numberWords } from "@/lib/words";
import {
  allTimeSentence,
  busiestMonthSentence,
  busiestYearSentence,
  rhythmSentence,
  shapesSentence,
  streakSentence,
  yearSentence,
} from "@/lib/year-sentences";
import { ShareYearDialog } from "./ShareYearDialog";

const SHAPES: Record<SessionShape, { label: string; range: string }> = {
  quick: { label: "Quick looks", range: `under ${formatHoursShort(SESSION_QUICK_MAX_MS)}` },
  short: { label: "Short", range: `${formatHoursShort(SESSION_QUICK_MAX_MS)} to ${formatHoursShort(SESSION_SHORT_MAX_MS)}` },
  plain: { label: "Medium", range: `${formatHoursShort(SESSION_SHORT_MAX_MS)} to ${formatHoursShort(SESSION_PLAIN_MAX_MS)}` },
  long: { label: "Long", range: `${formatHoursShort(SESSION_PLAIN_MAX_MS)} to ${formatHoursShort(SESSION_LONG_MAX_MS)}` },
  marathon: { label: "Marathons", range: `${formatHoursShort(SESSION_LONG_MAX_MS)} and more` },
};

function percent(part: number, whole: number): number {
  return whole > 0 ? Math.round((part / whole) * 100) : 0;
}

/** Whether the page shows one year or every day with play. */
type View = "year" | "all";

const VIEWS: Record<View, string> = { year: "Year", all: "All time" };

/** The bars of a year, one per month, named for the card with the year. */
function monthColumns(stats: YearStats): PeriodColumn[] {
  return stats.months.map((play, index) => {
    const month = new Date(stats.year, index, 1);
    return {
      short: formatDate(month, { month: "short" }),
      long: formatDate(month, { month: "long", year: "numeric" }),
      play,
    };
  });
}

function yearColumns(stats: AllTimeStats): PeriodColumn[] {
  return stats.years.map(({ year, play }) => ({ short: String(year), long: String(year), play }));
}

/** A year of play, or all of it, in numbers and sentences. */
export function StatsPage() {
  const { sessions, summaries, covers, today, loaded } = useLibrary();
  const playing = usePlaying().size > 0;
  const { accentHues, mode } = useAppearance();
  const navigate = useNavigate();
  const [params, setParams] = useSearchParams();
  // Opened from the button or the command palette, which also shows the year.
  const sharing = params.get("share") === "1";
  const setSharing = (open: boolean) => setParams(open ? { share: "1" } : {}, { replace: true });
  const [chosenView, setView] = useState<View>("year");
  const view: View = sharing ? "year" : chosenView;
  // 0 is this year, -1 the year before and so on.
  const [offset, setOffset] = useState(0);
  const [play, setPlay] = useState<{ key: string; play: YearPlay } | null>(null);
  const [playError, setPlayError] = useState<string | null>(null);
  const now = new Date();
  const year = now.getFullYear() + offset;
  const key = view === "all" ? "all" : String(year);
  // Reads again while a game runs and the window is seen, and when it shows again.
  const refreshed = useRefreshWhile(playing, LIVE_TOTALS_REFRESH_MS);

  // Read again whenever a session starts or ends.
  useEffect(() => {
    let cancelled = false;
    (key === "all" ? api.getAllTimePlay() : api.getYearPlay(Number(key)))
      .then((next) => {
        if (cancelled) return;
        setPlay({ key, play: next });
        setPlayError(null);
      })
      .catch((error) => {
        if (!cancelled) setPlayError(describeError(error));
      });
    return () => {
      cancelled = true;
    };
  }, [key, sessions, refreshed]);

  const byGame = useMemo(() => new Map(summaries.map((summary) => [summary.game.id, summary])), [summaries]);
  const titleOf = useCallback((gameId: string) => byGame.get(gameId)?.game.title ?? "a removed game", [byGame]);
  const shownPlay = play?.key === key ? play.play : null;
  // Worked out again when the play or the day changes, not each time the
  // share dialog opens or closes.
  const shown = useMemo(() => {
    if (!shownPlay) return null;
    const at = new Date();
    return key === "all"
      ? { all: allTimeStats(sessions, shownPlay, at), year: null, now: at }
      : { all: null, year: yearStats(sessions, Number(key), shownPlay), now: at };
    // eslint-disable-next-line react-hooks/exhaustive-deps -- a new day changes the year so far
  }, [sessions, key, shownPlay, today]);
  // A color per game for what is shown, the main color of its art, nudged
  // only where two of its games would look alike.
  const labelOf = useMemo(() => {
    const games = (shown?.all ?? shown?.year)?.games ?? [];
    const paletteOf = (gameId: string) => {
      const summary = byGame.get(gameId);
      return (summary?.tint ?? tintForTitle(summary?.game.title ?? "")).colors;
    };
    const yearMarks = markColors(games.map((game) => paletteOf(game.gameId)), accentHues, mode);
    const marks = new Map(games.map((game, index) => [game.gameId, yearMarks[index].color]));
    return (gameId: string) => ({ title: titleOf(gameId), color: marks.get(gameId) ?? "var(--soft)" });
  }, [shown, byGame, titleOf, accentHues, mode]);
  const openDay = useCallback((day: Date) => navigate(`/journal?day=${toDayKey(day)}`), [navigate]);

  const earliest = sessions.filter(countsAsPlay).reduce(
    (min, session) => Math.min(min, parseVaultimeDate(session.started_at_wall).getFullYear()),
    year,
  );

  if (!loaded) return null;
  if (playError) {
    return (
      <div className="px-8 pt-12 xl:px-14">
        <Notice tone="warning">
          {view === "all" ? "Your play" : "The year"} could not be read: {playError}
        </Notice>
      </div>
    );
  }
  if (!shown) return null;

  const { all, year: yearShown, now: shownAt } = shown;
  const stats = all ?? yearShown;
  const current = offset === 0;
  const top = stats.games[0];
  const sentence = all
    ? allTimeSentence({
        playedMs: all.playedMs,
        daysPlayed: all.daysPlayed,
        topTitle: top ? titleOf(top.gameId) : null,
        topMs: top?.runtimeMs ?? 0,
        gamesCount: all.games.length,
        since: all.firstDay,
      })
    : yearSentence({
        playedMs: stats.playedMs,
        daysPlayed: stats.daysPlayed,
        topTitle: top ? titleOf(top.gameId) : null,
        topMs: top?.runtimeMs ?? 0,
        gamesCount: stats.games.length,
        year,
        current,
      });
  const streak = stats.longestStreak;
  const longest = stats.longest;
  const rhythm = rhythmSentence(stats.weekClock);
  const idleGames = stats.games
    .filter((game) => game.runtimeMs >= STATS_IDLE_MIN_RUNTIME_MS && game.idleMs / game.runtimeMs >= STATS_IDLE_MIN_SHARE)
    .sort((a, b) => b.idleMs / b.runtimeMs - a.idleMs / a.runtimeMs)
    .slice(0, STATS_IDLE_GAMES);
  const daysOf = all
    ? {
        count: all.firstDay ? calendarDaysAgo(all.firstDay, startOfDay(shownAt, 1)) : 0,
        since: all.firstDay ? ` since ${formatDate(all.firstDay, { month: "short", year: "numeric" })}` : "",
      }
    : { count: daysSoFar(year, shownAt), since: current ? " so far" : "" };

  return (
    <div className="pb-16">
      <PageHeader
        overline={all ? (all.firstDay ? `Stats, since ${all.firstDay.getFullYear()}` : "Stats") : `Stats, ${year}`}
        title={all ? "All time" : current ? "This year" : offset === -1 ? "Last year" : String(year)}
        aside={
          <>
            <div role="group" aria-label="Show stats for" className="mr-1 flex rounded-full border border-hairline p-0.5">
              {(Object.keys(VIEWS) as View[]).map((each) => (
                <button
                  key={each}
                  type="button"
                  aria-pressed={view === each}
                  onClick={() => setView(each)}
                  className={cn(
                    "h-9 rounded-full px-4 text-[13px] transition-colors focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none",
                    view === each ? "bg-raised text-text" : "text-faint hover:text-soft",
                  )}
                >
                  {VIEWS[each]}
                </button>
              ))}
            </div>
            {!all && (
              <>
                <Button variant="outline" className="mr-1 h-11" onClick={() => setSharing(true)}>
                  <Share strokeWidth={1.8} />
                  Share your year
                </Button>
                <StepButton label="Previous year" disabled={earliest >= year} onClick={() => setOffset((value) => value - 1)}>
                  <ChevronLeft className="size-[18px]" strokeWidth={1.8} />
                </StepButton>
                <StepButton label="Next year" disabled={current} onClick={() => setOffset((value) => value + 1)}>
                  <ChevronRight className="size-[18px]" strokeWidth={1.8} />
                </StepButton>
              </>
            )}
          </>
        }
      >
        <PhraseText phrase={sentence} />
      </PageHeader>

      {yearShown && <ShareYearDialog open={sharing} onOpenChange={setSharing} stats={yearShown} now={shownAt} />}

      {stats.sessionsCount > 0 && (
        <>
          <StatTiles
            label={all ? "All time in numbers" : `${year} in numbers`}
            tiles={[
              {
                label: "Played",
                value: formatHoursMinutes(stats.playedMs),
                note: `across ${numberWords(stats.games.length)} game${stats.games.length === 1 ? "" : "s"}`,
              },
              {
                label: "Days played",
                value: String(stats.daysPlayed),
                note: `of ${daysOf.count} ${daysOf.count === 1 ? "day" : "days"}${daysOf.since}`,
                accent: true,
              },
              {
                label: "Longest streak",
                value: `${streak?.days ?? 0} ${streak?.days === 1 ? "day" : "days"}`,
                note: streak && streak.days > 1 ? formatDayRange(streak.start, streak.end) : "one day at a time",
              },
              {
                label: "Longest session",
                value: formatHoursMinutes(longest?.runtime_ms ?? 0),
                note: longest
                  ? `${titleOf(longest.game_id)}, ${
                      all
                        ? formatDate(parseVaultimeDate(longest.started_at_wall), {
                            day: "numeric",
                            month: "short",
                            year: "numeric",
                          })
                        : formatDayAndMonth(parseVaultimeDate(longest.started_at_wall))
                    }`
                  : "",
              },
            ]}
          />

          <div className="px-8 xl:px-14">
            {all ? (
              <PageSection title="Year by year" description={busiestYearSentence(all.years) ?? undefined}>
                <PeriodBars columns={yearColumns(all)} label="Active and idle time for each year" labelOf={labelOf} />
              </PageSection>
            ) : (
              <PageSection wide title="Every day" description={streakSentence(streak, stats.currentStreak, current)}>
                <YearHeatmap
                  year={year}
                  days={stats.days}
                  played={stats.played}
                  now={shownAt}
                  labelOf={labelOf}
                  onOpenDay={openDay}
                />
              </PageSection>
            )}

            <PageSection title="When you play" description={rhythm ? <PhraseText phrase={rhythm} /> : undefined}>
              <WeekClock weekClock={stats.weekClock} />
            </PageSection>

            <PageSection
              title="Games"
              description={
                top ? `${titleOf(top.gameId)} took ${percent(top.runtimeMs, stats.runtimeMs)} % of your playtime.` : undefined
              }
            >
              <div className="mb-3 flex justify-end">
                <ActiveIdleLegend />
              </div>
              <ol>
                {stats.games.slice(0, STATS_TOP_GAMES).map((game) => (
                  <GameRow
                    key={game.gameId}
                    game={game}
                    summary={byGame.get(game.gameId)}
                    cover={covers[game.gameId] ?? null}
                    topMs={top?.runtimeMs ?? 0}
                    share={percent(game.runtimeMs, stats.runtimeMs)}
                  />
                ))}
              </ol>
            </PageSection>

            {yearShown && (
              <PageSection title="Month by month" description={busiestMonthSentence(yearShown.months, year) ?? undefined}>
                <PeriodBars
                  columns={monthColumns(yearShown)}
                  label={`Active and idle time for each month of ${year}`}
                  labelOf={labelOf}
                />
              </PageSection>
            )}

            <PageSection title="Session lengths" description={shapesSentence(stats.shapes) ?? undefined}>
              <ShapeRows shapes={stats.shapes} />
            </PageSection>

            {idleGames.length > 0 && (
              <PageSection
                title="Left running"
                description="Games that kept running while you were away or busy in another window."
              >
                <ol>
                  {idleGames.map((game) => (
                    <li key={game.gameId} className="flex items-center gap-4 border-b border-rule py-3 last:border-b-0">
                      <Cover title={titleOf(game.gameId)} src={covers[game.gameId] ?? null} variant="tile" className="size-9 text-lg" />
                      <div className="min-w-0">
                        <GameLink gameId={game.gameId} title={titleOf(game.gameId)} known={byGame.has(game.gameId)} />
                        <div className="mt-0.5 text-[13px] text-faint">
                          Idle for {percent(game.idleMs, game.runtimeMs)} % of its {formatHoursShort(game.runtimeMs)}
                        </div>
                      </div>
                    </li>
                  ))}
                </ol>
              </PageSection>
            )}
          </div>
        </>
      )}
    </div>
  );
}

function GameLink({ gameId, title, known }: { gameId: string; title: string; known: boolean }) {
  if (!known) return <span className="block truncate text-[15px]">{title}</span>;
  return (
    <Link to={`/library/${gameId}`} className="block truncate text-[15px] transition-colors hover:text-violet">
      {title}
    </Link>
  );
}

function GameRow({
  game,
  summary,
  cover,
  topMs,
  share,
}: {
  game: GameYear;
  summary: GameSummary | undefined;
  cover: string | null;
  topMs: number;
  share: number;
}) {
  const title = summary?.game.title ?? "a removed game";
  return (
    <li className="grid grid-cols-[36px_minmax(0,1fr)_auto] items-center gap-x-4 border-b border-rule py-3 last:border-b-0">
      <Cover title={title} src={cover} variant="tile" className="size-9 text-lg" />
      <div className="min-w-0">
        <GameLink gameId={game.gameId} title={title} known={Boolean(summary)} />
        <div
          className="mt-2 flex h-1.5 gap-0.5"
          style={{ width: `${percent(game.runtimeMs, topMs)}%` }}
          title={`${formatHoursMinutes(game.activeMs)} active, ${formatHoursMinutes(game.idleMs)} idle`}
        >
          <span className="rounded-full bg-violet" style={{ width: `${percent(game.activeMs, game.runtimeMs)}%` }} />
          <span className="flex-1 rounded-full bg-idle" />
        </div>
      </div>
      <div className="text-right">
        <div className="font-mono text-sm tabular-nums">{formatHoursMinutes(game.runtimeMs)}</div>
        <div className="mt-0.5 text-xs text-faint tabular-nums">{share} %</div>
      </div>
    </li>
  );
}

function ShapeRows({ shapes }: { shapes: Record<SessionShape, number> }) {
  const max = Math.max(1, ...Object.values(shapes));
  return (
    <ol>
      {SESSION_SHAPES.map((shape) => (
        <li
          key={shape}
          className="grid grid-cols-[150px_minmax(0,1fr)_48px] items-center gap-x-4 border-b border-rule py-3 last:border-b-0"
        >
          <div>
            <div className="text-[15px]">{SHAPES[shape].label}</div>
            <div className="text-xs text-faint">{SHAPES[shape].range}</div>
          </div>
          <div className="h-1.5 rounded-full bg-violet/70" style={{ width: `${percent(shapes[shape], max)}%` }} />
          <div className="text-right text-sm tabular-nums">{shapes[shape]}</div>
        </li>
      ))}
    </ol>
  );
}
