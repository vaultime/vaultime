// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! Games that launchers register with Windows instead of keeping library
//! files: Battle.net, Rockstar, Riot, `HoYoPlay`, Ubisoft Connect, Legacy Games
//! and Big Fish. The registry is read first, the program of each game is
//! looked up on disk afterwards.

use std::collections::HashSet;
use std::fs;
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
    pub(crate) uninstall: String,
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
const PUBLISHERS: &[(&str, &[&str], &str)] =
    &[("Blizzard Entertainment", &["Battle.net"], "battlenet")];
const ROCKSTAR_LAUNCHER: &str = "rockstar games";
const ROCKSTAR_UNINSTALL: &str = "uninstall=";
const ROCKSTAR_LAUNCHER_ID: &str = "launcher";
const RIOT_GAME: &str = "Riot Game ";
const RIOT_CLIENT: &str = "riot";
const UBISOFT_INSTALL: &str = "Uplay Install ";

const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
const UNINSTALL_32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";
/// The global launcher and the one for mainland China.
const HOYOPLAY: [&str; 2] = [r"Software\Cognosphere\HYP", r"Software\miHoYo\HYP"];
const UBISOFT_INSTALLS: &str = r"SOFTWARE\WOW6432Node\Ubisoft\Launcher\Installs";
const LEGACY_GAMES: &str = r"Software\Legacy Games";
const BIG_FISH_GAMES: &str = r"SOFTWARE\WOW6432Node\Big Fish Games\Persistence\GameDB";
/// The Big Fish client lists itself among its games.
const BIG_FISH_CLIENT: &str = "F7315T1L1";

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

/// Rockstar Games Launcher games. Their uninstall command runs the launcher
/// with `uninstall=<title id>`, and the launcher's own id is `launcher`.
pub(crate) fn rockstar(programs: &[InstalledProgram]) -> Vec<Candidate> {
    programs
        .iter()
        .filter_map(|program| {
            let id = rockstar_title_id(&program.uninstall)?;
            let folder = clean_folder(program.folder.trim().trim_matches('"'));
            if id == ROCKSTAR_LAUNCHER_ID || folder.is_empty() {
                return None;
            }
            Some(Candidate {
                title: program.name.clone(),
                program: icon_program(&program.icon)
                    .filter(|icon| is_game_program_in(icon, &folder)),
                folder,
                source: "rockstar",
                source_id: id.to_owned(),
            })
        })
        .collect()
}

/// `"...\Rockstar Games\Launcher\Launcher.exe" -uninstall=gta5` gives `gta5`.
fn rockstar_title_id(uninstall: &str) -> Option<&str> {
    // ASCII lowercase keeps every byte where it is, so the indexes carry over.
    let lower = uninstall.to_ascii_lowercase();
    if !lower.contains(ROCKSTAR_LAUNCHER) {
        return None;
    }
    let start = lower.rfind(ROCKSTAR_UNINSTALL)? + ROCKSTAR_UNINSTALL.len();
    let id = uninstall[start..].trim().trim_matches('"');
    (!id.is_empty()).then_some(id)
}

/// What the Riot Client keeps about one installed product.
#[derive(Debug, Default)]
pub(crate) struct RiotProduct {
    pub(crate) id: String,
    pub(crate) folder: String,
    pub(crate) shortcut: Option<String>,
}

/// Riot games from their uninstall entries, "Riot Game <product>.<patchline>",
/// and from the Riot Client's product settings, since the uninstall entry can
/// be turned off.
pub(crate) fn riot(programs: &[InstalledProgram], products: &[RiotProduct]) -> Vec<Candidate> {
    let mut candidates: Vec<Candidate> = programs
        .iter()
        .filter_map(|program| {
            let product = program.key.strip_prefix(RIOT_GAME)?;
            (!is_riot_client(product) && !program.folder.is_empty()).then(|| Candidate {
                title: program.name.trim().to_owned(),
                folder: clean_folder(&program.folder),
                program: None,
                source: "riot",
                source_id: product.to_owned(),
            })
        })
        .collect();
    let known: HashSet<String> = candidates
        .iter()
        .map(|game| game.source_id.clone())
        .collect();
    let titles: Vec<(String, String)> = candidates
        .iter()
        .map(|game| (product_base(&game.source_id).to_owned(), game.title.clone()))
        .collect();
    candidates.extend(
        products
            .iter()
            .filter(|product| {
                !known.contains(&product.id)
                    && !is_riot_client(&product.id)
                    && !product.folder.is_empty()
            })
            .map(|product| Candidate {
                title: riot_title(product, &titles),
                folder: clean_folder(&product.folder),
                program: None,
                source: "riot",
                source_id: product.id.clone(),
            }),
    );
    candidates
}

/// The shortcut name, else the title of another patchline of the same game
/// with this one's name, "Teamfight Tactics (PBE)", else the product id.
fn riot_title(product: &RiotProduct, titles: &[(String, String)]) -> String {
    if let Some(stem) = product
        .shortcut
        .as_deref()
        .and_then(|name| Path::new(name).file_stem())
        .filter(|stem| !stem.is_empty())
    {
        return stem.to_string_lossy().into_owned();
    }
    let base = product_base(&product.id);
    let patchline = product.id[base.len()..].trim_start_matches('.');
    titles.iter().find(|(other, _)| other == base).map_or_else(
        || product.id.clone(),
        |(_, title)| format!("{title} ({})", patchline.to_uppercase()),
    )
}

/// `league_of_legends.pbe` gives `league_of_legends`.
fn product_base(product: &str) -> &str {
    product.split_once('.').map_or(product, |(base, _)| base)
}

fn is_riot_client(product: &str) -> bool {
    product.to_ascii_lowercase().starts_with(RIOT_CLIENT)
}

/// A top level `key: "value"` line of a YAML file.
fn yaml_value(text: &str, key: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
        .map(|value| value.trim().trim_matches('"').to_owned())
        .filter(|value| !value.is_empty())
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
    candidates.extend(rockstar(&programs));
    candidates.extend(riot(&programs, &riot_products()));
    candidates.extend(hoyoplay(&hoyoplay_installs(), &programs));
    candidates.extend(ubisoft(&ubisoft_installs(), &programs));
    candidates.extend(legacy_games());
    candidates.extend(big_fish_games());
    candidates
}

/// Legacy Games keeps a key per game with its title, folder and program.
fn legacy_games() -> Vec<Candidate> {
    let Ok(games) = CURRENT_USER.open(LEGACY_GAMES) else {
        return Vec::new();
    };
    subkeys(&games)
        .into_iter()
        .filter_map(|id| {
            let game = games.open(&id).ok()?;
            let folder = text(&game, "InstDir");
            let program = text(&game, "GameExe");
            (!folder.is_empty() && !program.is_empty()).then(|| Candidate {
                title: text(&game, "ProductName"),
                program: Some(
                    Path::new(&folder)
                        .join(program)
                        .to_string_lossy()
                        .into_owned(),
                ),
                folder: clean_folder(&folder),
                source: "legacy",
                source_id: id,
            })
        })
        .filter(|candidate| !candidate.title.is_empty())
        .collect()
}

/// Big Fish keeps a key per game with its title and program.
fn big_fish_games() -> Vec<Candidate> {
    let Ok(games) = LOCAL_MACHINE.open(BIG_FISH_GAMES) else {
        return Vec::new();
    };
    subkeys(&games)
        .into_iter()
        .filter(|sku| sku != BIG_FISH_CLIENT)
        .filter_map(|sku| {
            let game = games.open(&sku).ok()?;
            let program = text(&game, "ExecutablePath");
            let folder = Path::new(&program).parent()?.to_string_lossy().into_owned();
            let title = text(&game, "Name");
            (!title.is_empty()).then_some(Candidate {
                title,
                folder,
                program: Some(program),
                source: "bigfish",
                source_id: sku,
            })
        })
        .collect()
}

fn text(key: &Key, name: &str) -> String {
    key.get_string(name).unwrap_or_default()
}

fn subkeys(key: &Key) -> Vec<String> {
    key.keys().map(Iterator::collect).unwrap_or_default()
}

pub(crate) fn installed_programs() -> Vec<InstalledProgram> {
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
                    uninstall: text(&entry, "UninstallString"),
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
    for key in HOYOPLAY {
        if let Ok(root) = CURRENT_USER.open(key) {
            collect_game_paths(&root, "", REGISTRY_SEARCH_DEPTH, &mut found);
        }
    }
    found
}

/// The product settings the Riot Client writes for every install, in
/// `ProgramData\Riot Games\Metadata\<product>\<product>.product_settings.yaml`.
fn riot_products() -> Vec<RiotProduct> {
    let Some(program_data) = std::env::var_os("ProgramData") else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(
        PathBuf::from(program_data)
            .join("Riot Games")
            .join("Metadata"),
    ) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let id = entry.file_name().to_string_lossy().into_owned();
            let settings = entry.path().join(format!("{id}.product_settings.yaml"));
            let text = fs::read_to_string(settings).ok()?;
            Some(RiotProduct {
                folder: yaml_value(&text, "product_install_full_path")?,
                shortcut: yaml_value(&text, "shortcut_name"),
                id,
            })
        })
        .collect()
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
            uninstall: String::new(),
        }
    }

    fn with_uninstall(mut program: InstalledProgram, uninstall: &str) -> InstalledProgram {
        program.uninstall = uninstall.into();
        program
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
        assert_eq!(titles, ["Diablo IV"]);
        assert_eq!(games[0].source, "battlenet");
        assert_eq!(
            games[0].program.as_deref(),
            Some(r"E:\BlizzardLibrary\Diablo IV\Diablo IV.exe")
        );
    }

    #[test]
    fn riot_skips_the_client_and_anti_cheat() {
        let games = riot(&sample(), &[]);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "VALORANT");
        assert_eq!(games[0].source_id, "valorant.live");
        assert!(!games[0].folder.ends_with(MAIN_SEPARATOR));
    }

    #[test]
    fn riot_adds_installs_without_an_uninstall_entry() {
        let products = [
            RiotProduct {
                id: "valorant.live".into(),
                folder: "E:/RiotLibrary/Riot Games/VALORANT/live".into(),
                shortcut: Some("VALORANT.lnk".into()),
            },
            RiotProduct {
                id: "league_of_legends.pbe".into(),
                folder: "E:/RiotLibrary/Riot Games/League of Legends (PBE)".into(),
                shortcut: Some("League of Legends (PBE).lnk".into()),
            },
            RiotProduct {
                id: "valorant.pbe".into(),
                folder: "C:/Riot Games/VALORANT/pbe".into(),
                shortcut: None,
            },
            RiotProduct {
                id: "Riot Client".into(),
                folder: "C:/Riot Games/Riot Client".into(),
                shortcut: None,
            },
        ];
        let games = riot(&sample(), &products);
        let titles: Vec<&str> = games.iter().map(|game| game.title.as_str()).collect();
        assert_eq!(
            titles,
            ["VALORANT", "League of Legends (PBE)", "VALORANT (PBE)"]
        );
    }

    #[test]
    fn reads_top_level_yaml_values() {
        let yaml = "patching_policy: \"manual\"\nproduct_install_full_path: \"E:/Riot/VALORANT/live\"\nsettings:\n    create_uninstall_key: true\n";
        assert_eq!(
            yaml_value(yaml, "product_install_full_path").as_deref(),
            Some("E:/Riot/VALORANT/live")
        );
        assert_eq!(
            yaml_value(yaml, "create_uninstall_key"),
            None,
            "nested keys are no top level keys"
        );
    }

    #[test]
    fn rockstar_reads_the_title_id_from_the_uninstall_command() {
        let launcher = r#""C:\Program Files\Rockstar Games\Launcher\Launcher.exe""#;
        let programs = [
            with_uninstall(
                program(
                    "Rockstar Games Launcher",
                    "Rockstar Games Launcher",
                    "",
                    r"C:\Program Files\Rockstar Games\Launcher",
                    "",
                ),
                &format!("{launcher} -uninstall=launcher"),
            ),
            with_uninstall(
                program(
                    "{5EFC6C07-6B87-43FC-9524-F9E967241741}",
                    "Grand Theft Auto V",
                    "Rockstar Games",
                    r#""D:\Games\GTAV""#,
                    r#""D:\Games\GTAV\PlayGTAV.exe""#,
                ),
                &format!("{launcher} -uninstall=gta5"),
            ),
            with_uninstall(
                program("Other", "Other App", "Someone", r"C:\Other", ""),
                r"C:\Other\uninstall.exe /uninstall=other",
            ),
        ];
        let games = rockstar(&programs);
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].title, "Grand Theft Auto V");
        assert_eq!(games[0].source_id, "gta5");
        assert_eq!(games[0].folder, r"D:\Games\GTAV");
        assert_eq!(
            games[0].program.as_deref(),
            Some(r"D:\Games\GTAV\PlayGTAV.exe")
        );
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
