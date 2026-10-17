// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a day's strip in the journal draws: runs of play that overlap on the
//! clock, the games in each part of them, and the games that ran side by
//! side.

use std::collections::BTreeSet;

use serde::Serialize;

/// A session's part on a day, where it ran on the clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StripPart {
    pub session_id: String,
    /// The game it counts as.
    pub game_id: String,
    /// Milliseconds since 1970 in UTC.
    pub start_ms: i64,
    pub end_ms: i64,
    /// The session ran in from the day before.
    pub open_start: bool,
    /// The session runs on past midnight.
    pub open_end: bool,
}

/// A part of a run in which the same games ran.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RunPiece {
    pub start_ms: i64,
    pub end_ms: i64,
    /// In the order the games joined the run, so a game keeps its place while
    /// others come and go.
    pub game_ids: Vec<String>,
    /// The first start and the last end of the parts running in it.
    pub sessions_start_ms: i64,
    pub sessions_end_ms: i64,
}

/// Parts that overlap on the clock, with no gap in between.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlayRun {
    pub start_ms: i64,
    pub end_ms: i64,
    /// From start to end, split where a game joins or leaves.
    pub pieces: Vec<RunPiece>,
}

/// Games that ran side by side with each other, or with a game they both ran
/// beside.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SideBySideGroup {
    /// In the order the games joined.
    pub game_ids: Vec<String>,
    /// The game in every stretch, when more than two games are in the group.
    pub alongside: Option<String>,
    /// Clock time with at least two of the games running.
    pub ms: i64,
    /// The most games running at the same time.
    pub most_at_once: usize,
}

/// Calls `visit` for each stretch between two neighboring starts or ends in
/// which parts ran, in clock order, with the indices of those parts in the
/// order they started. Parts without length run nowhere.
fn sweep(parts: &[&StripPart], mut visit: impl FnMut(i64, i64, &[usize])) {
    let mut by_start: Vec<usize> = (0..parts.len())
        .filter(|&index| parts[index].end_ms > parts[index].start_ms)
        .collect();
    by_start.sort_by_key(|&index| parts[index].start_ms);
    let mut bounds: Vec<i64> = by_start
        .iter()
        .flat_map(|&index| [parts[index].start_ms, parts[index].end_ms])
        .collect();
    bounds.sort_unstable();
    bounds.dedup();
    let mut running: Vec<usize> = Vec::new();
    let mut next = 0;
    for pair in bounds.windows(2) {
        let (start, end) = (pair[0], pair[1]);
        running.retain(|&index| parts[index].end_ms > start);
        while next < by_start.len() && parts[by_start[next]].start_ms <= start {
            running.push(by_start[next]);
            next += 1;
        }
        if !running.is_empty() {
            visit(start, end, &running);
        }
    }
}

/// The parts of a day as runs of overlapping play, in clock order. Parts
/// that only touch stay apart. Two copies of one game running at once are
/// one game here, as they count once. `parts` come in the order they began,
/// which sets the order of games that start together.
pub fn play_runs(parts: &[StripPart]) -> Vec<PlayRun> {
    let mut sorted: Vec<&StripPart> = parts.iter().collect();
    sorted.sort_by_key(|part| part.start_ms);
    let mut groups: Vec<Vec<&StripPart>> = Vec::new();
    let mut group_end = 0;
    for part in sorted {
        match groups.last_mut() {
            Some(group) if part.start_ms < group_end => {
                group.push(part);
                group_end = group_end.max(part.end_ms);
            }
            _ => {
                groups.push(vec![part]);
                group_end = part.end_ms;
            }
        }
    }
    groups
        .into_iter()
        .map(|group| {
            let start_ms = group[0].start_ms;
            let end_ms = group
                .iter()
                .map(|part| part.end_ms)
                .max()
                .unwrap_or(start_ms);
            let mut order: Vec<&str> = Vec::new();
            for part in &group {
                if !order.contains(&part.game_id.as_str()) {
                    order.push(&part.game_id);
                }
            }
            let mut pieces: Vec<RunPiece> = Vec::new();
            sweep(&group, |start, end, running| {
                let games: BTreeSet<&str> = running
                    .iter()
                    .map(|&index| group[index].game_id.as_str())
                    .collect();
                let game_ids: Vec<String> = order
                    .iter()
                    .filter(|game| games.contains(*game))
                    .map(|game| (*game).to_owned())
                    .collect();
                let first = running
                    .iter()
                    .map(|&index| group[index].start_ms)
                    .min()
                    .unwrap_or(start);
                let last = running
                    .iter()
                    .map(|&index| group[index].end_ms)
                    .max()
                    .unwrap_or(end);
                match pieces.last_mut() {
                    Some(piece) if piece.game_ids == game_ids => {
                        piece.end_ms = end;
                        piece.sessions_start_ms = piece.sessions_start_ms.min(first);
                        piece.sessions_end_ms = piece.sessions_end_ms.max(last);
                    }
                    _ => pieces.push(RunPiece {
                        start_ms: start,
                        end_ms: end,
                        game_ids,
                        sessions_start_ms: first,
                        sessions_end_ms: last,
                    }),
                }
            });
            // A part without time on the clock still gets a piece, so it shows.
            if pieces.is_empty() {
                pieces.push(RunPiece {
                    start_ms,
                    end_ms,
                    game_ids: order
                        .iter()
                        .take(1)
                        .map(|game| (*game).to_owned())
                        .collect(),
                    sessions_start_ms: start_ms,
                    sessions_end_ms: end_ms,
                });
            }
            PlayRun {
                start_ms,
                end_ms,
                pieces,
            }
        })
        .collect()
}

/// The stretches of `runs` in which games ran side by side, grouped by the
/// games they share, in clock order.
pub fn side_by_side(runs: &[PlayRun]) -> Vec<SideBySideGroup> {
    let shared: Vec<&RunPiece> = runs
        .iter()
        .flat_map(|run| &run.pieces)
        .filter(|piece| piece.game_ids.len() > 1)
        .collect();
    let mut groups: Vec<Vec<&RunPiece>> = Vec::new();
    for stretch in shared {
        let shares_game = |group: &Vec<&RunPiece>| {
            group.iter().any(|other| {
                other
                    .game_ids
                    .iter()
                    .any(|game| stretch.game_ids.contains(game))
            })
        };
        let Some(first) = groups.iter().position(shares_game) else {
            groups.push(vec![stretch]);
            continue;
        };
        let mut joined: Vec<&RunPiece> = groups
            .iter()
            .filter(|group| shares_game(group))
            .flatten()
            .copied()
            .collect();
        joined.push(stretch);
        joined.sort_by_key(|piece| piece.start_ms);
        let mut kept: Vec<Vec<&RunPiece>> = groups[..first].to_vec();
        kept.push(joined);
        kept.extend(
            groups[first + 1..]
                .iter()
                .filter(|group| !shares_game(group))
                .cloned(),
        );
        groups = kept;
    }
    groups
        .into_iter()
        .map(|stretches| {
            let mut game_ids: Vec<String> = Vec::new();
            for game in stretches.iter().flat_map(|stretch| &stretch.game_ids) {
                if !game_ids.contains(game) {
                    game_ids.push(game.clone());
                }
            }
            let in_every = game_ids
                .iter()
                .find(|game| {
                    stretches
                        .iter()
                        .all(|stretch| stretch.game_ids.contains(game))
                })
                .cloned();
            SideBySideGroup {
                alongside: if game_ids.len() > 2 { in_every } else { None },
                ms: stretches
                    .iter()
                    .map(|stretch| stretch.end_ms - stretch.start_ms)
                    .sum(),
                most_at_once: stretches
                    .iter()
                    .map(|stretch| stretch.game_ids.len())
                    .max()
                    .unwrap_or(0),
                game_ids,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;

    /// A part of `game` from `hour:minute` for `minutes`, on a day from 0.
    fn part(game: &str, hour: i64, minute: i64, minutes: i64) -> StripPart {
        let start_ms = (hour * 60 + minute) * MINUTE;
        StripPart {
            session_id: format!("{game}-{hour}-{minute}"),
            game_id: game.into(),
            start_ms,
            end_ms: start_ms + minutes * MINUTE,
            open_start: false,
            open_end: false,
        }
    }

    fn games_of(runs: &[PlayRun]) -> Vec<Vec<String>> {
        runs.iter()
            .map(|run| {
                run.pieces
                    .iter()
                    .map(|piece| piece.game_ids.join("+"))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn puts_a_launcher_and_the_matches_inside_it_into_one_run() {
        let runs = play_runs(&[
            part("client", 18, 57, 129),
            part("match", 19, 1, 35),
            part("match", 19, 37, 41),
            part("match", 20, 26, 39),
        ]);
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].start_ms, (18 * 60 + 57) * MINUTE);
        assert_eq!(runs[0].end_ms, (21 * 60 + 6) * MINUTE);
        assert_eq!(
            games_of(&runs)[0],
            [
                "client",
                "client+match",
                "client",
                "client+match",
                "client",
                "client+match",
                "client"
            ]
        );
        // The client's pieces carry the whole session, so its fill spans all of it.
        assert_eq!(runs[0].pieces[2].sessions_start_ms, runs[0].start_ms);
        assert_eq!(runs[0].pieces[2].sessions_end_ms, runs[0].end_ms);
    }

    #[test]
    fn keeps_parts_that_only_touch_apart() {
        let runs = play_runs(&[part("a", 10, 0, 60), part("b", 11, 0, 60)]);
        assert_eq!(games_of(&runs), [["a"], ["b"]]);
    }

    #[test]
    fn draws_two_copies_of_one_game_as_one_game() {
        let runs = play_runs(&[part("game", 18, 0, 60), part("game", 18, 30, 60)]);
        assert_eq!(games_of(&runs), [["game"]]);
        assert!(side_by_side(&runs).is_empty());
    }

    #[test]
    fn gives_a_part_without_time_on_the_clock_a_piece() {
        let runs = play_runs(&[part("a", 10, 0, 0)]);
        assert_eq!(runs[0].pieces.len(), 1);
    }

    #[test]
    fn joins_a_stretch_while_the_same_games_run_and_splits_when_a_third_joins() {
        let runs = play_runs(&[
            part("a", 10, 0, 120),
            part("b", 10, 30, 60),
            part("c", 11, 0, 15),
        ]);
        let shared: Vec<String> = runs[0]
            .pieces
            .iter()
            .filter(|piece| piece.game_ids.len() > 1)
            .map(|piece| piece.game_ids.concat())
            .collect();
        assert_eq!(shared, ["ab", "abc", "ab"]);
    }

    #[test]
    fn keeps_one_stretch_when_a_game_starts_again_while_the_other_runs() {
        let runs = play_runs(&[
            part("a", 10, 0, 120),
            part("b", 10, 30, 120),
            part("a", 12, 0, 60),
        ]);
        let groups = side_by_side(&runs);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].ms, 120 * MINUTE);
    }

    #[test]
    fn keeps_two_pairs_that_never_met_apart() {
        let groups = side_by_side(&play_runs(&[
            part("a", 13, 0, 120),
            part("b", 14, 0, 120),
            part("c", 20, 0, 180),
            part("d", 21, 0, 60),
        ]));
        let games: Vec<Vec<String>> = groups.iter().map(|group| group.game_ids.clone()).collect();
        assert_eq!(games, [["a", "b"], ["c", "d"]]);
        assert!(groups.iter().all(|group| group.ms == 60 * MINUTE));
    }

    #[test]
    fn names_the_game_that_ran_through_every_stretch() {
        let groups = side_by_side(&play_runs(&[
            part("a", 9, 0, 600),
            part("b", 11, 0, 150),
            part("c", 13, 0, 120),
            part("d", 17, 0, 60),
        ]));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].game_ids, ["a", "b", "c", "d"]);
        assert_eq!(groups[0].alongside.as_deref(), Some("a"));
        assert_eq!(groups[0].most_at_once, 3);
        assert_eq!(groups[0].ms, (150 + 90 + 60) * MINUTE);
    }

    #[test]
    fn joins_games_that_met_through_another_game() {
        let groups = side_by_side(&play_runs(&[
            part("a", 10, 0, 60),
            part("b", 10, 30, 60),
            part("c", 11, 15, 60),
            part("d", 12, 0, 60),
        ]));
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].game_ids, ["a", "b", "c", "d"]);
        assert_eq!(groups[0].alongside, None);
        assert_eq!(groups[0].most_at_once, 2);
    }
}
