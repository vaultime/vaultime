// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Games that launchers register with Windows instead of keeping library
//! files: Battle.net, Riot, `HoYoPlay` and Ubisoft Connect. The registry is read
//! first, the program of each game is looked up on disk afterwards.

use std::collections::HashSet;
use std::path::{MAIN_SEPARATOR, MAIN_SEPARATOR_STR, Path, PathBuf};

use windows_registry::{CURRENT_USER, Key, LOCAL_MACHINE};

use super::{DiscoveredGame, find_main_executable, metadata};
use crate::constants::REGISTRY_SEARCH_DEPTH;
use crate::platform::process::path_key;

/// One entry of the Windows list of installed programs.
#[derive(Debug, Default, Clone)]
pub(crate) struct InstalledProgram {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) publisher: String,
    pub(crate) folder: String,
    pub(crate) icon: String,
}

/// A game a launcher registered, before its program is looked up on disk.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Candidate {
    pub(crate) title: String,
    pub(crate) folder: String,
    /// The game program, when the launcher names it.
    pub(crate) program: Option<String>,
    pub(crate) source: &'static str,
    pub(crate) source_id: String,
}

/// Launchers that register their games under their own publisher name, with
/// the launcher's own entries to skip and the source label.
const PUBLISHERS: &[(&str, &[&str], &str)] = &[
    ("Blizzard Entertainment", &["Battle.net"], "battlenet"),
    ("Electronic Arts", &["EA app", "Origin"], "ea"),
    (
        "Rockstar Games",
        &["Rockstar Games Launcher", "Rockstar Games Social Club"],
        "rockstar",
    ),
];
const RIOT_GAME: &str = "Riot Game ";
const RIOT_CLIENT: &str = "Riot_Client";
const UBISOFT_INSTALL: &str = "Uplay Install ";

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
const HOYOPLAY: &str = r"Software\Cognosphere\HYP";
const UBISOFT_INSTALLS: &str = r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs";

/// Games of launchers that register them under their publisher name. Their
/// uninstall entries often name the game program as icon.
pub(crate) fn by_publisher(programs: &[InstalledProgram]) -> Vec<Candidate> {
    programs
        .iter()
        .filter_map(|program| {
            let (_, launchers, source) = PUBLISHERS
                .iter()
                .find(|(publisher, _, _)| program.publisher.starts_with(publisher))?;
            if launchers.contains(&program.name.as_str()) || program.folder.is_empty() {
                return None;
            }
            let folder = clean_folder(&program.folder);
            Some(Candidate {
                title: program.name.clone(),
                program: icon_program(&program.icon)
                    .filter(|icon| is_game_program_in(icon, &folder)),
                folder,
                source,
                source_id: program.key.clone(),
            })
        })
        .collect()
}

/// Uninstallers make good icons too, only a game program in the folder counts.
fn is_game_program_in(program: &str, folder: &str) -> bool {
    let path = Path::new(program);
    path_key(program).starts_with(&path_key(folder))
        && path
            .file_name()
            .is_some_and(|name| metadata::is_likely_game_executable(&name.to_string_lossy()))
}

/// Riot games, registered per product as "Riot Game <product>.<patchline>".
pub(crate) fn riot(programs: &[InstalledProgram]) -> Vec<Candidate> {
    programs
        .iter()
        .filter_map(|program| {
            let product = program.key.strip_prefix(RIOT_GAME)?;
            (!product.starts_with(RIOT_CLIENT) && !program.folder.is_empty()).then(|| Candidate {
                title: program.name.clone(),
                folder: clean_folder(&program.folder),
                program: None,
                source: "riot",
                source_id: product.to_owned(),
            })
        })
        .collect()
}

/// `HoYoPlay` games from their game ids and install folders. The title comes
/// from the uninstall entry that names the same game id.
pub(crate) fn hoyoplay(
    installs: &[(String, String)],
    programs: &[InstalledProgram],
) -> Vec<Candidate> {
    installs
        .iter()
        .map(|(game, folder)| Candidate {
            title: programs
                .iter()
                .find(|program| program.key.contains(game.as_str()) && !program.name.is_empty())
                .map_or_else(|| folder_title(folder), |program| program.name.clone()),
            folder: clean_folder(folder),
            program: None,
            source: "hoyoplay",
            source_id: game.clone(),
        })
        .collect()
}

/// Ubisoft Connect games from their ids and install folders, titled by the
/// "Uplay Install <id>" uninstall entry.
pub(crate) fn ubisoft(
    installs: &[(String, String)],
    programs: &[InstalledProgram],
) -> Vec<Candidate> {
    installs
        .iter()
        .map(|(id, folder)| Candidate {
            title: programs
                .iter()
                .find(|program| {
                    program.key.strip_prefix(UBISOFT_INSTALL) == Some(id.as_str())
                        && !program.name.is_empty()
                })
                .map_or_else(|| folder_title(folder), |program| program.name.clone()),
            folder: clean_folder(folder),
            program: None,
            source: "ubisoft",
            source_id: id.clone(),
        })
        .collect()
}

/// The program an icon value points to, `"C:\Game\Game.exe",0` gives
/// `C:\Game\Game.exe`. Icon files are no programs.
fn icon_program(icon: &str) -> Option<String> {
    let icon = icon.trim().trim_matches('"');
    let path = match icon.rsplit_once(',') {
        Some((path, index)) if index.trim().parse::<i32>().is_ok() => path,
        _ => icon,
    }
    .trim()
    .trim_matches('"');
    path.to_ascii_lowercase()
        .ends_with(".exe")
        .then(|| path.to_owned())
}

/// Some launchers write folders with forward slashes and a trailing one.
fn clean_folder(folder: &str) -> String {
    let folder = folder.replace('/', MAIN_SEPARATOR_STR);
    let trimmed = folder.trim_end_matches(MAIN_SEPARATOR);
    if trimmed.is_empty() {
        folder
    } else {
        trimmed.to_owned()
    }
}

fn folder_title(folder: &str) -> String {
    Path::new(&clean_folder(folder)).file_name().map_or_else(
        || folder.to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

/// Looks up the game program on disk. A program the launcher named wins,
/// otherwise the most likely one in the install folder.
fn resolve(candidate: Candidate, existing: &HashSet<String>) -> Option<DiscoveredGame> {
    let program = candidate
        .program
        .map(PathBuf::from)
        .filter(|program| program.is_file())
        .or_else(|| find_main_executable(Path::new(&candidate.folder), &candidate.title))?;
    let executable_path = program.to_string_lossy().into_owned();
    Some(DiscoveredGame {
        title: candidate.title,
        already_added: existing.contains(&path_key(&executable_path)),
        executable_path,
        install_folder: Some(candidate.folder),
        source: candidate.source.into(),
        source_id: Some(candidate.source_id),
    })
}

/// Every registered game whose program exists. Games that share a program,
/// like League of Legends and Teamfight Tactics, are listed once.
pub(crate) fn discover(existing: &HashSet<String>) -> Vec<DiscoveredGame> {
    let candidates = candidates();
    let mut seen = HashSet::new();
    let games: Vec<DiscoveredGame> = candidates
        .into_iter()
        .filter_map(|candidate| resolve(candidate, existing))
        .filter(|game| seen.insert(path_key(&game.executable_path)))
        .collect();
    log::info!("registered launcher games found: {}", games.len());
    games
}

/// Every registered game, read from the registry only.
fn candidates() -> Vec<Candidate> {
    let programs = installed_programs();
    let mut candidates = by_publisher(&programs);
    candidates.extend(riot(&programs));
    candidates.extend(hoyoplay(&hoyoplay_installs(), &programs));
    candidates.extend(ubisoft(&ubisoft_installs(), &programs));
    candidates
}

fn text(key: &Key, name: &str) -> String {
    key.get_string(name).unwrap_or_default()
}

fn subkeys(key: &Key) -> Vec<String> {
    key.keys().map(Iterator::collect).unwrap_or_default()
}

fn installed_programs() -> Vec<InstalledProgram> {
    [
        (LOCAL_MACHINE, UNINSTALL),
        (LOCAL_MACHINE, UNINSTALL_32),
        (CURRENT_USER, UNINSTALL),
    ]
    .into_iter()
    .filter_map(|(root, path)| root.open(path).ok())
    .flat_map(|list| {
        subkeys(&list)
            .into_iter()
            .filter_map(|name| {
                let entry = list.open(&name).ok()?;
                Some(InstalledProgram {
                    name: text(&entry, "DisplayName"),
                    publisher: text(&entry, "Publisher"),
                    folder: text(&entry, "InstallLocation"),
                    icon: text(&entry, "DisplayIcon"),
                    key: name,
                })
            })
            .collect::<Vec<_>>()
    })
    .collect()
}

/// Game ids and install folders. Games sit a level or two below the `HoYoPlay`
/// key, test builds of standalone launchers a few levels deeper.
fn hoyoplay_installs() -> Vec<(String, String)> {
    let mut found = Vec::new();
    if let Ok(root) = CURRENT_USER.open(HOYOPLAY) {
        collect_game_paths(&root, "", REGISTRY_SEARCH_DEPTH, &mut found);
    }
    found
}

fn collect_game_paths(key: &Key, name: &str, depth: usize, found: &mut Vec<(String, String)>) {
    let folder = text(key, "GameInstallPath");
    if !folder.is_empty() {
        found.push((name.to_owned(), folder));
    }
    if depth == 0 {
        return;
    }
    for child in subkeys(key) {
        if let Ok(sub) = key.open(&child) {
            collect_game_paths(&sub, &child, depth - 1, found);
        }
    }
}

fn ubisoft_installs() -> Vec<(String, String)> {
    let Ok(installs) = LOCAL_MACHINE.open(UBISOFT_INSTALLS) else {
        return Vec::new();
    };
    subkeys(&installs)
        .into_iter()
        .filter_map(|id| {
            let folder = text(&installs.open(&id).ok()?, "InstallDir");
            (!folder.is_empty()).then_some((id, folder))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn program(
        key: &str,
        name: &str,
        publisher: &str,
        folder: &str,
        icon: &str,
    ) -> InstalledProgram {
        InstalledProgram {
            key: key.into(),
            name: name.into(),
            publisher: publisher.into(),
            folder: folder.into(),
            icon: icon.into(),
        }
    }

    fn sample() -> Vec<InstalledProgram> {
        vec![
            program(
                "{EA-APP}",
                "EA app",
                "Electronic Arts",
                r"C:\Program Files\Electronic Arts\EA Desktop",
                "",
            ),
            program(
                "{SOME-EA-GAME}",
                "Some EA Game",
                "Electronic Arts",
                r"D:\EA Games\Some EA Game",
                r"D:\EA Games\Some EA Game\unins000.exe",
            ),
            program(
                "Rockstar Games Launcher",
                "Rockstar Games Launcher",
                "Rockstar Games",
                r"C:\Program Files\Rockstar Games\Launcher",
                "",
            ),
            program(
                "Battle.net",
                "Battle.net",
                "Blizzard Entertainment",
                r"C:\Program Files (x86)\Battle.net",
                "",
            ),
            program(
                "Diablo IV",
                "Diablo IV",
                "Blizzard Entertainment",
                r"E:\BlizzardLibrary\Diablo IV",
                r#""E:\BlizzardLibrary\Diablo IV\Diablo IV.exe",0"#,
            ),
            program(
                "Riot Game valorant.live",
                "VALORANT",
                "Riot Games, Inc",
                "E:/RiotLibrary/Riot Games/VALORANT/live",
                "",
            ),
            program(
                "Riot Game Riot_Client.",
                "Riot Client",
                "Riot Games, Inc",
                "C:/Riot Games/Riot Client",
                "",
            ),
            program(
                "Riot Vanguard",
                "Riot Vanguard",
                "Riot Games, Inc.",
                r"C:\Program Files\Riot Vanguard",
                "",
            ),
            program(
                "hk4e_global_1_0_x_production",
                "Genshin Impact",
                "COGNOSPHERE PTE. LTD.",
                r"C:\Program Files\HoYoPlay",
                "",
            ),
            program("Uplay Install 1081", "Far Cry 6", "Ubisoft", "", ""),
            program("Some Tool", "Some Tool", "Someone", r"C:\Tools", ""),
        ]
    }

    #[test]
    fn publishers_list_games_but_not_their_launchers() {
        let games = by_publisher(&sample());
        let titles: Vec<&str> = games.iter().map(|game| game.title.as_str()).collect();
        assert_eq!(titles, ["Some EA Game", "Diablo IV"]);
        assert_eq!(games[0].source, "ea");
        assert_eq!(games[0].program, None, "an uninstaller is no game program");
        assert_eq!(games[1].source, "battlenet");
        assert_eq!(
            games[1].program.as_deref(),
            Some(r"E:\BlizzardLibrary\Diablo IV\Diablo IV.exe")
        );
    }

    #[test]
    fn riot_skips_the_client_and_anti_cheat() {
        let games = riot(&sample());
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "VALORANT");
        assert_eq!(games[0].source_id, "valorant.live");
        assert!(!games[0].folder.ends_with(MAIN_SEPARATOR));
    }

    #[test]
    fn hoyoplay_takes_titles_from_uninstall_entries() {
        let installs = [
            (
                "hk4e_global".to_owned(),
                r"C:\Program Files\HoYoPlay\games\Genshin Impact game".to_owned(),
            ),
            (
                "nap_global".to_owned(),
                "C:/Program Files/HoYoPlay/games/ZenlessZoneZero Game".to_owned(),
            ),
        ];
        let games = hoyoplay(&installs, &sample());
        assert_eq!(games[0].title, "Genshin Impact");
        assert_eq!(
            games[1].title, "ZenlessZoneZero Game",
            "falls back to the folder"
        );
    }

    #[test]
    fn ubisoft_titles_by_install_id() {
        let installs = [("1081".to_owned(), "C:/Games/Ubisoft/Far Cry 6/".to_owned())];
        let games = ubisoft(&installs, &sample());
        assert_eq!(games[0].title, "Far Cry 6");
        assert_eq!(games[0].folder, clean_folder("C:/Games/Ubisoft/Far Cry 6"));
    }

    #[test]
    fn icons_name_programs_only() {
        assert_eq!(
            icon_program(r#""C:\Game\Game.exe",0"#).as_deref(),
            Some(r"C:\Game\Game.exe")
        );
        assert_eq!(
            icon_program(r"C:\Game\Game.EXE").as_deref(),
            Some(r"C:\Game\Game.EXE")
        );
        assert_eq!(
            icon_program(r"C:\ProgramData\Riot Games\valorant.ico"),
            None
        );
        assert_eq!(icon_program(""), None);
    }

    /// Prints what the registry says about launcher games on this PC, without
    /// touching the game folders: `cargo test -- --ignored --nocapture registered`.
    #[test]
    #[ignore = "reads the registry of this PC"]
    fn report_registered_games() {
        for candidate in candidates() {
            println!("{candidate:?}");
        }
    }
}
