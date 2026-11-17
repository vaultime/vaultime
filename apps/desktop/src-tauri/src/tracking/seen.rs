// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Programs from the game folders, the usual install folders of the launchers
//! that the folder scan reads, that run in front without being a game of the
//! library. The tracker adds up how long each one had the foreground, so
//! Discover can offer the games the library misses. It notes nothing about a
//! program outside those folders, an ignored one, or anything while a game
//! of the library is in front. Steam libraries are left out, as Steam
//! discovery finds their games, and a Windows game that Wine runs shows as
//! Wine and is left out too.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use log::warn;

use crate::constants::{POLL_INTERVAL, SEEN_ROOTS_REFRESH, SEEN_RUNNING_FLUSH};
use crate::db::connection::Database;
use crate::db::models::Game;
use crate::db::repo::seen;
use crate::discovery::metadata::{infer_title, is_likely_game_executable};
use crate::discovery::scanner::default_scan_paths;
use crate::platform::activity::ActivitySnapshot;
use crate::platform::process::{RunningProcess, path_key};

#[derive(Default)]
pub struct SeenPrograms {
    /// The game folders as `path_key` gives them, each ending in a separator.
    roots: Vec<String>,
    roots_read: Option<Instant>,
    /// Seconds in front not written yet, with the path, by path key.
    unwritten: HashMap<String, (String, u64)>,
}

impl SeenPrograms {
    #[cfg(test)]
    fn in_folders(folders: &[PathBuf]) -> Self {
        Self {
            roots: keys_of(folders),
            roots_read: Some(Instant::now()),
            unwritten: HashMap::new(),
        }
    }

    /// Counts one tick for the program in front, when it lies in a game
    /// folder, looks like a game and is no game of the library.
    pub fn note(
        &mut self,
        db: &Database,
        processes: &[RunningProcess],
        snapshot: &ActivitySnapshot,
        game_in_front: bool,
        library: &[Game],
    ) {
        if game_in_front || !snapshot.foreground_known {
            return;
        }
        let Some(path) = snapshot.foreground_pid.and_then(|pid| {
            processes
                .iter()
                .find(|process| process.pid == pid)?
                .exe_path
                .clone()
        }) else {
            return;
        };
        let key = path_key(&path);
        // Steam discovery finds the games of every Steam library, wherever it lies.
        if key
            .split(['/', '\\'])
            .any(|part| part.eq_ignore_ascii_case("steamapps"))
        {
            return;
        }
        if self
            .roots_read
            .is_none_or(|read| read.elapsed() >= SEEN_ROOTS_REFRESH)
        {
            let folders: Vec<PathBuf> = default_scan_paths()
                .into_iter()
                .map(PathBuf::from)
                .collect();
            self.roots = keys_of(&folders);
            self.roots_read = Some(Instant::now());
        }
        let in_game_folder = self.roots.iter().any(|root| key.starts_with(root));
        let looks_like_game = Path::new(&path)
            .file_name()
            .is_some_and(|name| is_likely_game_executable(&name.to_string_lossy()));
        if !in_game_folder
            || !looks_like_game
            || library.iter().any(|game| {
                game.executable_path
                    .as_deref()
                    .is_some_and(|executable| path_key(executable) == key)
            })
        {
            return;
        }
        let entry = self.unwritten.entry(key).or_insert_with(|| (path, 0));
        entry.1 += POLL_INTERVAL.as_secs();
        if entry.1 < SEEN_RUNNING_FLUSH.as_secs() {
            return;
        }
        let seconds = std::mem::take(&mut entry.1);
        let seconds = i64::try_from(seconds).unwrap_or(i64::MAX);
        if let Err(error) = seen::add_front_time(db, &entry.0, &infer_title(&entry.0), seconds) {
            warn!("could not note a program seen running: {error}");
        }
    }
}

/// `path_key` of each folder with a separator at its end, so a folder never
/// matches another one that only starts with its name.
fn keys_of(folders: &[PathBuf]) -> Vec<String> {
    folders
        .iter()
        .map(|folder| {
            let mut key = path_key(&folder.to_string_lossy());
            if !key.ends_with(['/', '\\']) {
                key.push(std::path::MAIN_SEPARATOR);
            }
            key
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::SEEN_RUNNING_MIN;
    use crate::playtime::testing::database;

    fn front(pid: u32) -> ActivitySnapshot {
        ActivitySnapshot {
            foreground_pid: Some(pid),
            foreground_supported: true,
            foreground_known: true,
            idle_for: None,
            idle_supported: false,
        }
    }

    fn process(pid: u32, path: &Path) -> RunningProcess {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        RunningProcess::new(pid, name, Some(path.to_string_lossy().into_owned()), None)
    }

    /// Ticks for `minutes` with the program of `pid` in front.
    fn run(
        seen: &mut SeenPrograms,
        db: &Database,
        processes: &[RunningProcess],
        minutes: u64,
        game_in_front: bool,
    ) {
        for _ in 0..minutes * 60 / POLL_INTERVAL.as_secs() {
            seen.note(db, processes, &front(1), game_in_front, &[]);
        }
    }

    fn offered(db: &Database) -> Vec<String> {
        seen::list(db, i64::try_from(SEEN_RUNNING_MIN.as_secs()).unwrap())
            .unwrap()
            .into_iter()
            .map(|program| program.title)
            .collect()
    }

    #[test]
    fn notes_a_game_that_runs_in_front_from_a_game_folder() {
        let db = database();
        let games = Path::new("/games");
        let mut seen = SeenPrograms::in_folders(&[games.to_path_buf()]);
        let tunic = [process(1, &games.join("Tunic").join("Tunic.exe"))];
        run(&mut seen, &db, &tunic, 5, false);
        assert!(offered(&db).is_empty(), "five minutes are not enough");
        run(&mut seen, &db, &tunic, 5, false);
        assert_eq!(offered(&db), ["Tunic"]);
    }

    #[test]
    fn notes_nothing_outside_the_game_folders_or_while_a_game_is_in_front() {
        let db = database();
        let games = Path::new("/games");
        let mut seen = SeenPrograms::in_folders(&[games.to_path_buf()]);
        run(
            &mut seen,
            &db,
            &[process(1, Path::new("/apps/Mail/Mail.exe"))],
            30,
            false,
        );
        run(
            &mut seen,
            &db,
            &[process(1, Path::new("/games-old/Tunic/Tunic.exe"))],
            30,
            false,
        );
        run(
            &mut seen,
            &db,
            &[process(1, &games.join("Tunic").join("Tunic.exe"))],
            30,
            true,
        );
        run(
            &mut seen,
            &db,
            &[process(1, &games.join("Tunic").join("setup.exe"))],
            30,
            false,
        );
        let steam = games.join("SteamLibrary").join("steamapps").join("common");
        run(
            &mut seen,
            &db,
            &[process(1, &steam.join("Tunic").join("Tunic.exe"))],
            30,
            false,
        );
        assert!(offered(&db).is_empty());
    }
}
