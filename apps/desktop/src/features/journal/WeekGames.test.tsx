// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

import { fireEvent, render, screen } from "@testing-library/react";
import { MemoryRouter } from "react-router";
import { describe, expect, it, vi } from "vitest";
import type { GameSummary } from "@/features/library/library-context";
import { HOUR_MS, JOURNAL_WEEK_GAMES, MINUTE_MS } from "@/lib/constants";
import { WeekGames } from "./WeekGames";

const byGame = new Map(
  Array.from({ length: JOURNAL_WEEK_GAMES + 2 }, (_, index) => {
    const id = String(index);
    return [id, { game: { id, title: `Game ${id}` }, cover: null } as unknown as GameSummary];
  }),
);

function show(props: Partial<Parameters<typeof WeekGames>[0]>) {
  return render(
    <MemoryRouter>
      <WeekGames games={[]} byGame={byGame} fillOf={() => "red"} longestDay={null} onDay={() => {}} {...props} />
    </MemoryRouter>,
  );
}

describe("WeekGames", () => {
  it("lists the most played games with their hours and counts the rest", () => {
    const games = [...byGame.keys()].map((id, index) => ({
      game_id: id,
      runtime_ms: (20 - index) * HOUR_MS,
      active_ms: 0,
      idle_ms: 0,
    }));
    show({ games });
    expect(screen.getAllByRole("link")).toHaveLength(JOURNAL_WEEK_GAMES);
    expect(screen.getByRole("link", { name: /Game 0/ }).getAttribute("href")).toBe("/library/0");
    expect(screen.getByText("20h 00m")).toBeTruthy();
    expect(screen.getByText("and two more")).toBeTruthy();
  });

  it("goes to the longest day", () => {
    const onDay = vi.fn();
    show({
      games: [{ game_id: "0", runtime_ms: HOUR_MS, active_ms: HOUR_MS, idle_ms: 0 }],
      longestDay: { day: "2026-11-14", playedMs: 348 * MINUTE_MS },
      onDay,
    });
    fireEvent.click(screen.getByRole("button", { name: /Longest day/ }));
    expect(onDay).toHaveBeenCalledWith("2026-11-14");
    expect(screen.getByText("5h 48m")).toBeTruthy();
  });

  it("shows nothing for a week without games", () => {
    const { container } = show({});
    expect(container.textContent).toBe("");
  });
});
