// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! The look for new games when Vaultime starts. It reads Steam and the other
//! launchers but not the folder scan, whose finds are guesses, and keeps
//! what the player has never been shown until the window can offer it.

use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};

use log::{info, warn};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::constants::{AUTO_DISCOVER_SETTING, AUTO_DISCOVERY_DELAY, FOUND_GAMES_EVENT};
use crate::db::connection::Database;
use crate::db::repo::{games, ignored, offered, settings};
use crate::error::Result;
use crate::platform::process::path_key;

use super::DiscoveredGame;

/// Games the look at start found, until the window offers them.
#[derive(Default)]
pub struct FoundGames {
    pending: Mutex<Vec<DiscoveredGame>>,
}

impl FoundGames {
    fn set(&self, found: Vec<DiscoveredGame>) {
        *self.pending.lock().unwrap_or_else(PoisonError::into_inner) = found;
    }

    fn get(&self) -> Vec<DiscoveredGame> {
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// What the window may offer right now.
#[derive(Debug, Serialize)]
pub struct NewGames {
    pub games: Vec<DiscoveredGame>,
    /// True when discovery showed the player games on this PC before, so
    /// these are new since then.
    pub since_last: bool,
}

/// What a candidate is compared with to tell whether it is new.
#[derive(Debug, Default)]
struct Known {
    executables: HashSet<String>,
    folders: HashSet<String>,
    ignored: HashSet<String>,
    offered: HashSet<String>,
}

impl Known {
    fn load(db: &Database) -> Result<Self> {
        let mut known = Self {
            ignored: ignored::keys(db)?,
            offered: offered::keys(db)?,
            ..Self::default()
        };
        for game in games::list_local_games(db)? {
            known
                .executables
                .extend(game.executable_path.as_deref().map(path_key));
            known
                .folders
                .extend(game.install_folder.as_deref().map(path_key));
        }
        Ok(known)
    }
}

/// Whether the player turned the look at start off.
pub fn enabled(db: &Database) -> bool {
    settings::get_setting(db, AUTO_DISCOVER_SETTING)
        .ok()
        .flatten()
        .is_none_or(|value| value != "false")
}

/// Looks for new games once, `AUTO_DISCOVERY_DELAY` after start, on a thread
/// of its own, and tells the window when it found some.
pub fn start(app: AppHandle, db: Arc<Database>) {
    let spawned = std::thread::Builder::new()
        .name("vaultime-auto-discovery".into())
        .spawn(move || {
            std::thread::sleep(AUTO_DISCOVERY_DELAY);
            if !enabled(&db) {
                return;
            }
            match find(&db) {
                Ok(found) if found.is_empty() => info!("no new games found at start"),
                Ok(found) => {
                    info!("found {} new game(s) at start", found.len());
                    app.state::<FoundGames>().set(found);
                    let _ = app.emit(FOUND_GAMES_EVENT, ());
                }
                Err(error) => warn!("looking for new games at start failed: {error}"),
            }
        });
    if let Err(error) = spawned {
        warn!("could not start looking for new games: {error}");
    }
}

/// Games from Steam and the other launchers the player has never been shown.
fn find(db: &Database) -> Result<Vec<DiscoveredGame>> {
    let found = from_launchers(
        super::steam::discover_steam_games(db),
        super::discover_launcher_games(db),
    )?;
    super::learn_launcher_ids(db, &found);
    still_new(db, found)
}

/// Both lists, or the one that worked when the other failed. Fails only
/// when both did.
fn from_launchers(
    steam: Result<Vec<DiscoveredGame>>,
    launchers: Result<Vec<DiscoveredGame>>,
) -> Result<Vec<DiscoveredGame>> {
    match (steam, launchers) {
        (Ok(mut steam), Ok(launchers)) => {
            steam.extend(launchers);
            Ok(steam)
        }
        (Ok(games), Err(error)) | (Err(error), Ok(games)) => {
            warn!("part of the look for new games failed: {error}");
            Ok(games)
        }
        (Err(error), Err(other)) => {
            warn!("reading the other launchers failed: {other}");
            Err(error)
        }
    }
}

/// The candidates that are new: not in the library by executable or install
/// folder, not ignored and never shown before, each executable once.
fn still_new(db: &Database, candidates: Vec<DiscoveredGame>) -> Result<Vec<DiscoveredGame>> {
    Ok(filter_new(candidates, &Known::load(db)?))
}

fn filter_new(candidates: Vec<DiscoveredGame>, known: &Known) -> Vec<DiscoveredGame> {
    let mut seen = HashSet::new();
    candidates
        .into_iter()
        .filter(|game| {
            let exe = path_key(&game.executable_path);
            let in_library = game.already_added
                || known.executables.contains(&exe)
                || game
                    .install_folder
                    .as_deref()
                    .is_some_and(|folder| known.folders.contains(&path_key(folder)));
            !in_library
                && !known.ignored.contains(&exe)
                && !known.offered.contains(&exe)
                && seen.insert(exe)
        })
        .collect()
}

/// The games found at start that are still new. The library may have
/// changed since, so they are checked again, and the ones no longer new are
/// dropped for good.
pub fn new_games(db: &Database, found: &FoundGames) -> Result<NewGames> {
    let pending = found.get();
    if pending.is_empty() {
        return Ok(NewGames {
            games: Vec::new(),
            since_last: false,
        });
    }
    let known = Known::load(db)?;
    let since_last = !known.offered.is_empty();
    let games = filter_new(pending, &known);
    found.set(games.clone());
    // Marked as they are shown, against the library of that moment.
    let games = super::mark_same_games(db, games)?;
    Ok(NewGames { games, since_last })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::CreateGame;
    use crate::error::VaultimeError;

    fn candidate(exe: &str, folder: &str) -> DiscoveredGame {
        DiscoveredGame {
            title: exe.into(),
            executable_path: exe.into(),
            install_folder: Some(folder.into()),
            source: "steam".into(),
            source_id: None,
            already_added: false,
            same_game: None,
        }
    }

    fn exes(games: &[DiscoveredGame]) -> Vec<&str> {
        games
            .iter()
            .map(|game| game.executable_path.as_str())
            .collect()
    }

    #[test]
    fn keeps_only_what_the_player_has_not_seen() {
        let known = Known {
            executables: HashSet::from([path_key("/lib/a/a.exe")]),
            folders: HashSet::from([path_key("/lib/b")]),
            ignored: HashSet::from([path_key("/lib/c/c.exe")]),
            offered: HashSet::from([path_key("/lib/d/d.exe")]),
        };
        let mut added = candidate("/lib/e/e.exe", "/lib/e");
        added.already_added = true;

        let new = filter_new(
            vec![
                candidate("/lib/a/a.exe", "/lib/a"),
                candidate("/lib/b/other.exe", "/lib/b"),
                candidate("/lib/c/c.exe", "/lib/c"),
                candidate("/lib/d/d.exe", "/lib/d"),
                added,
                candidate("/lib/f/f.exe", "/lib/f"),
                candidate("/lib/f/f.exe", "/elsewhere"),
                candidate("/lib/g/g.exe", "/lib/g"),
            ],
            &known,
        );

        assert_eq!(exes(&new), ["/lib/f/f.exe", "/lib/g/g.exe"]);
        assert_eq!(new[0].install_folder.as_deref(), Some("/lib/f"));
    }

    #[test]
    fn one_failing_launcher_keeps_the_others() {
        let failed = || Err(VaultimeError::Invalid("unreadable".into()));
        let found = || Ok(vec![candidate("/lib/a/a.exe", "/lib/a")]);

        assert_eq!(from_launchers(found(), found()).unwrap().len(), 2);
        assert_eq!(from_launchers(failed(), found()).unwrap().len(), 1);
        assert_eq!(from_launchers(found(), failed()).unwrap().len(), 1);
        assert!(from_launchers(failed(), failed()).is_err());
    }

    #[test]
    fn offers_each_game_once_and_follows_the_library() {
        let db = Database::open_in_memory().unwrap();
        let found = FoundGames::default();
        assert!(new_games(&db, &found).unwrap().games.is_empty());

        found.set(vec![
            candidate("/lib/a/a.exe", "/lib/a"),
            candidate("/lib/b/b.exe", "/lib/b"),
            candidate("/lib/c/c.exe", "/lib/c"),
        ]);
        // Meanwhile the player added one game and ignored another.
        games::create_game(
            &db,
            &CreateGame {
                title: "A".into(),
                executable_path: Some("/lib/a/a.exe".into()),
                install_folder: Some("/lib/a".into()),
                launcher_source: Some("steam".into()),
            },
        )
        .unwrap();
        ignored::ignore(&db, "/lib/b/b.exe", "B").unwrap();

        let first = new_games(&db, &found).unwrap();
        assert_eq!(exes(&first.games), ["/lib/c/c.exe"]);
        assert!(!first.since_last);

        // Shown once, never again, also not on the next start.
        offered::remember(&db, &["/lib/c/c.exe".into()]).unwrap();
        assert!(new_games(&db, &found).unwrap().games.is_empty());
        let next_start = still_new(
            &db,
            vec![
                candidate("/lib/c/c.exe", "/lib/c"),
                candidate("/lib/d/d.exe", "/lib/d"),
            ],
        )
        .unwrap();
        assert_eq!(exes(&next_start), ["/lib/d/d.exe"]);

        found.set(next_start);
        assert!(new_games(&db, &found).unwrap().since_last);
    }

    #[test]
    fn is_on_unless_turned_off() {
        let db = Database::open_in_memory().unwrap();
        assert!(enabled(&db));
        settings::set_setting(&db, AUTO_DISCOVER_SETTING, "false").unwrap();
        assert!(!enabled(&db));
        settings::set_setting(&db, AUTO_DISCOVER_SETTING, "true").unwrap();
        assert!(enabled(&db));
    }
}
