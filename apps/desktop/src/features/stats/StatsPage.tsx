// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useState } from "react";
import { Link } from "react-router";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { DayBarsLegend } from "@/components/charts/DayBars";
import { MonthBars } from "@/components/charts/MonthBars";
import { WeekClock } from "@/components/charts/WeekClock";
import { YearHeatmap } from "@/components/charts/YearHeatmap";
import { PageHeader, PageSection, StepButton } from "@/components/layout/Page";
import { StatTiles } from "@/components/layout/StatTiles";
import { Cover } from "@/components/media/Cover";
import { PhraseText } from "@/components/media/PhraseText";
import { useLibrary, type GameSummary } from "@/features/library/library-context";
import {
  SESSION_LONG_MAX_MS,
  SESSION_PLAIN_MAX_MS,
  SESSION_QUICK_MAX_MS,
  SESSION_SHORT_MAX_MS,
  STATS_IDLE_GAMES,
  STATS_IDLE_MIN_RUNTIME_MS,
  STATS_IDLE_MIN_SHARE,
  STATS_TOP_GAMES,
} from "@/lib/constants";
import {
  busiestMonthSentence,
  rhythmSentence,
  shapesSentence,
  streakSentence,
  yearSentence,
} from "@/lib/sentences";
import { daysSoFar, SESSION_SHAPES, yearStats, type GameYear, type SessionShape } from "@/lib/stats";
import { formatDayAndMonth, formatDayRange, formatHoursMinutes, formatHoursShort, parseVaultimeDate } from "@/lib/time";
import { numberWords } from "@/lib/words";

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

/** A year of play in numbers and sentences. */
export function StatsPage() {
  const { sessions, summaries, covers, loaded } = useLibrary();
  // 0 is this year, -1 the year before and so on.
  const [offset, setOffset] = useState(0);
  const now = new Date();
  const year = now.getFullYear() + offset;
  const stats = yearStats(sessions, year, now);
  const byGame = new Map(summaries.map((summary) => [summary.game.id, summary]));
  const titleOf = (gameId: string) => byGame.get(gameId)?.game.title ?? "a removed game";

  const earliest = sessions.reduce(
    (min, session) => Math.min(min, parseVaultimeDate(session.started_at_wall).getFullYear()),
    year,
  );

  if (!loaded) return null;

  const current = offset === 0;
  const top = stats.games[0];
  const sentence = yearSentence({
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

  return (
    <div className="pb-16">
      <PageHeader
        overline={`Stats, ${year}`}
        title={current ? "This year" : offset === -1 ? "Last year" : String(year)}
        aside={
          <>
            <StepButton label="Previous year" disabled={earliest >= year} onClick={() => setOffset((value) => value - 1)}>
              <ChevronLeft className="size-[18px]" strokeWidth={1.8} />
            </StepButton>
            <StepButton label="Next year" disabled={current} onClick={() => setOffset((value) => value + 1)}>
              <ChevronRight className="size-[18px]" strokeWidth={1.8} />
            </StepButton>
          </>
        }
      >
        <PhraseText phrase={sentence} />
      </PageHeader>

      {stats.sessionsCount > 0 && (
        <>
          <StatTiles
            label={`${year} in numbers`}
            tiles={[
              {
                label: "Played",
                value: formatHoursMinutes(stats.playedMs),
                note: `across ${numberWords(stats.games.length)} game${stats.games.length === 1 ? "" : "s"}`,
              },
              {
                label: "Days played",
                value: String(stats.daysPlayed),
                note: `of ${daysSoFar(year, now)} ${current ? "so far" : "days"}`,
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
                  ? `${titleOf(longest.game_id)}, ${formatDayAndMonth(parseVaultimeDate(longest.started_at_wall))}`
                  : "",
              },
            ]}
          />

          <div className="px-8 xl:px-14">
            <PageSection title="Every day" description={streakSentence(streak, stats.currentStreak, current)}>
              <YearHeatmap year={year} activeByDay={stats.activeByDay} now={now} />
            </PageSection>

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
                <DayBarsLegend />
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

            <PageSection title="Month by month" description={busiestMonthSentence(stats.months, year) ?? undefined}>
              <MonthBars months={stats.months} year={year} />
            </PageSection>

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
