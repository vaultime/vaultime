// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { useEffect, useState } from "react";
import { useNavigate } from "react-router";
import { ActiveIdleLegend, PlayBars, type PlayPoint } from "@/components/charts/PlayBars";
import { YearHeatmap } from "@/components/charts/YearHeatmap";
import { useAppearance } from "@/features/appearance/appearance-context";
import { useLibrary } from "@/features/library/library-context";
import { GAME_HISTORY_DAYS, GAME_HISTORY_MONTHS, LIVE_TOTALS_REFRESH_MS } from "@/lib/constants";
import { markColors, type GameTint } from "@/lib/game-tint";
import { groupByBucket } from "@/lib/play-totals";
import { toDayKey } from "@/lib/session-stats";
import * as api from "@/lib/tauri";
import { parseVaultimeDate, startOfWeek, UI_LOCALE } from "@/lib/time";
import type { PlayBucket, PlayTotal, Session } from "@/lib/types";
import { useRefreshWhile } from "@/lib/use-refresh-while";
import { cn, describeError } from "@/lib/utils";
import { numberWords } from "@/lib/words";

type Range = "days" | "months" | "years" | "calendar";

const RANGES: Record<Range, string> = {
  days: `${GAME_HISTORY_DAYS} days`,
  months: `${GAME_HISTORY_MONTHS} months`,
  years: "Years",
  calendar: "Every day",
};

function monthKey(date: Date): string {
  return `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}`;
}

/** What a range asks the core for: from, to and the bucket. */
function queryOf(range: Range, now: Date, firstYear: number): [string, string, PlayBucket] {
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate());
  const tomorrow = toDayKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() + 1));
  switch (range) {
    case "days":
      return [
        toDayKey(new Date(today.getFullYear(), today.getMonth(), today.getDate() - (GAME_HISTORY_DAYS - 1))),
        tomorrow,
        "day",
      ];
    case "months":
      return [
        toDayKey(new Date(today.getFullYear(), today.getMonth() - (GAME_HISTORY_MONTHS - 1), 1)),
        toDayKey(new Date(today.getFullYear(), today.getMonth() + 1, 1)),
        "month",
      ];
    case "years":
      return [`${firstYear}-01-01`, `${today.getFullYear() + 1}-01-01`, "year"];
    case "calendar":
      return [`${today.getFullYear()}-01-01`, `${today.getFullYear() + 1}-01-01`, "day"];
  }
}

/** The columns of a range, oldest first, with nothing played filled in as zero. */
function pointsOf(range: Exclude<Range, "calendar">, totals: PlayTotal[], now: Date, firstYear: number): PlayPoint[] {
  const byKey = new Map(totals.map((total) => [total.bucket, total]));
  const point = (key: string, label: string, name: string): PlayPoint => ({
    key,
    label,
    name,
    activeMs: byKey.get(key)?.active_ms ?? 0,
    idleMs: byKey.get(key)?.idle_ms ?? 0,
  });
  if (range === "days") {
    return Array.from({ length: GAME_HISTORY_DAYS }, (_, index) => {
      const day = new Date(now.getFullYear(), now.getMonth(), now.getDate() - (GAME_HISTORY_DAYS - 1 - index));
      return point(
        toDayKey(day),
        day.toLocaleDateString(UI_LOCALE, { day: "numeric", month: "short" }),
        day.toLocaleDateString(UI_LOCALE, { weekday: "long", day: "numeric", month: "long" }),
      );
    });
  }
  if (range === "months") {
    return Array.from({ length: GAME_HISTORY_MONTHS }, (_, index) => {
      const month = new Date(now.getFullYear(), now.getMonth() - (GAME_HISTORY_MONTHS - 1 - index), 1);
      return point(
        monthKey(month),
        month.toLocaleDateString(UI_LOCALE, { month: "short" }),
        month.toLocaleDateString(UI_LOCALE, { month: "long", year: "numeric" }),
      );
    });
  }
  return Array.from({ length: now.getFullYear() - firstYear + 1 }, (_, index) => {
    const year = String(firstYear + index);
    return point(year, year, year);
  });
}

/**
 * A game's play over the last days, months or years, or every day of this
 * year, from the core's quarter hour slices.
 */
export function GameHistory({
  gameId,
  sessions,
  tint,
  title,
}: {
  gameId: string;
  /** The game's sessions, so the history reloads when one starts or ends. */
  sessions: Session[];
  tint: GameTint;
  title: string;
}) {
  const navigate = useNavigate();
  const { accentHues, mode } = useAppearance();
  const color = markColors([tint.colors], accentHues, mode)[0].color;
  const { active } = useLibrary();
  const refreshed = useRefreshWhile(
    active.some((session) => session.game_id === gameId),
    LIVE_TOTALS_REFRESH_MS,
  );
  const [range, setRange] = useState<Range>("days");
  const [loaded, setLoaded] = useState<{ range: Range; totals: PlayTotal[] } | null>(null);
  const [error, setError] = useState<string | null>(null);
  const now = new Date();
  const firstYear = sessions.reduce(
    (year, session) => Math.min(year, parseVaultimeDate(session.started_at_wall).getFullYear()),
    now.getFullYear(),
  );

  useEffect(() => {
    let cancelled = false;
    const [from, to, bucket] = queryOf(range, new Date(), firstYear);
    api
      .getPlayTotals(from, to, bucket, gameId)
      .then((totals) => {
        if (cancelled) return;
        setLoaded({ range, totals });
        setError(null);
      })
      .catch((loadError) => {
        if (!cancelled) setError(describeError(loadError));
      });
    return () => {
      cancelled = true;
    };
  }, [range, gameId, firstYear, sessions, refreshed]);

  const totals = loaded?.range === range ? loaded.totals : null;
  return (
    <section aria-labelledby="history-title" className="flex flex-col gap-3.5">
      <div className="flex flex-wrap items-baseline justify-between gap-3">
        <h2 id="history-title" className="font-display text-[30px]">
          History
        </h2>
        <div className="flex items-center gap-4">
          {range !== "calendar" && <ActiveIdleLegend />}
          <div role="group" aria-label="Show history by" className="flex rounded-full border border-hairline p-0.5">
            {(Object.keys(RANGES) as Range[]).map((key) => (
              <button
                key={key}
                type="button"
                aria-pressed={range === key}
                onClick={() => setRange(key)}
                className={cn(
                  "h-7 rounded-full px-3 text-[12px] transition-colors focus-visible:ring-2 focus-visible:ring-violet/60 focus-visible:outline-none",
                  range === key ? "bg-raised text-text" : "text-faint hover:text-soft",
                )}
              >
                {RANGES[key]}
              </button>
            ))}
          </div>
        </div>
      </div>
      {error && <p className="text-sm text-amber">The history could not be read: {error}</p>}
      {totals &&
        (range === "calendar" ? (
          <YearHeatmap
            year={now.getFullYear()}
            days={groupByBucket(totals)}
            now={now}
            labelOf={() => ({ title, color })}
            onOpenDay={(day) => navigate(`/journal?week=${toDayKey(startOfWeek(day))}`)}
          />
        ) : (
          <PlayBars
            points={pointsOf(range, totals, now, firstYear)}
            label={
              range === "days"
                ? `Active and idle time on each of the last ${numberWords(GAME_HISTORY_DAYS)} days`
                : range === "months"
                  ? `Active and idle time in each of the last ${numberWords(GAME_HISTORY_MONTHS)} months`
                  : "Active and idle time in each year"
            }
            axis={range === "days" ? "ends" : "all"}
            lastLabel={range === "days" ? "Today" : undefined}
            empty={range === "days" ? "Not played on any of these days." : "Not played in this time."}
          />
        ))}
    </section>
  );
}
