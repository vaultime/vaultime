// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Ubisoft Connect's cache of game settings, `cache\configuration\
//! configurations` in its folder. Each record holds a product's settings as
//! YAML, with the game's name and the programs it starts.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::Path;

use super::protobuf::{Value, fields, number, text};

const CACHE_FILE: &str = r"cache\configuration\configurations";

/// What the cache says about a product.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Product {
    pub(crate) name: Option<String>,
    /// The programs it starts, relative to the install folder or a folder in it.
    pub(crate) programs: Vec<String>,
}

/// The cached products of the launcher in `launcher_folder`, by install id.
pub(crate) fn read_cache(launcher_folder: &Path) -> HashMap<String, Product> {
    std::fs::read(launcher_folder.join(CACHE_FILE))
        .map(|data| parse(&data))
        .unwrap_or_default()
}

/// Each record (1) with its product id (1), install id (2), 0 for add-ons,
/// and its settings (3). A pre-order or an edition shares the install of its
/// game, whose own record, with the install id as product id, counts.
pub(crate) fn parse(data: &[u8]) -> HashMap<String, Product> {
    let Some(records) = fields(data) else {
        return HashMap::new();
    };
    let mut cache: HashMap<String, (bool, Product)> = HashMap::new();
    for record in records.iter().filter_map(|(field, value)| match value {
        Value::Bytes(record) if *field == 1 => fields(record),
        _ => None,
    }) {
        let product_id = number(&record, 1);
        let install_id = number(&record, 2).filter(|id| *id != 0);
        let (Some(id), Some(settings)) = (install_id.or(product_id), text(&record, 3)) else {
            continue;
        };
        let own = install_id.is_none() || install_id == product_id;
        match cache.entry(id.to_string()) {
            Entry::Vacant(slot) => {
                slot.insert((own, read_settings(settings)));
            }
            Entry::Occupied(mut slot) if own && !slot.get().0 => {
                slot.insert((own, read_settings(settings)));
            }
            Entry::Occupied(_) => {}
        }
    }
    cache
        .into_iter()
        .map(|(id, (_, product))| (id, product))
        .collect()
}

/// The name under `root`, which may be a key of the default localization,
/// and every `relative` program of an `executables` entry.
fn read_settings(yaml: &str) -> Product {
    let mut section = "";
    let mut name = None;
    let mut defaults: HashMap<&str, String> = HashMap::new();
    let mut in_default = false;
    let mut after_entry = false;
    let mut programs: Vec<String> = Vec::new();
    for line in yaml.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent == 0 {
            section = trimmed.split(':').next().unwrap_or_default();
            continue;
        }
        match section {
            "root" => {
                if indent == 2
                    && name.is_none()
                    && let Some(value) = trimmed.strip_prefix("name:")
                {
                    name = Some(unquote(value));
                }
                if after_entry
                    && let Some(program) = trimmed.strip_prefix("relative:").map(unquote)
                    && !program.is_empty()
                    && !programs.contains(&program)
                {
                    programs.push(program);
                }
                after_entry = trimmed == "- path:";
            }
            "localizations" if indent == 2 => in_default = trimmed == "default:",
            "localizations" if in_default && indent == 4 => {
                if let Some((key, value)) = trimmed.split_once(':') {
                    defaults.insert(key.trim(), unquote(value));
                }
            }
            _ => {}
        }
    }
    let name = name
        .map(|name| defaults.get(name.as_str()).cloned().unwrap_or(name))
        .filter(|name| !name.is_empty());
    Product { name, programs }
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|inner| inner.strip_suffix('\''))
        })
        .unwrap_or(value)
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::super::protobuf::write;
    use super::*;

    /// Far Cry 4's settings as the cache holds them, shortened.
    const FAR_CRY_4: &str = "version: 2.0\r\nroot:\r\n  name: GAMENAME\r\n  sort_string: Far Cry 4\r\n  start_game:\r\n    online:\r\n      executables:\r\n        - path:\r\n            relative: FarCry4.exe\r\n          working_directory:\r\n            register: HKEY_LOCAL_MACHINE\\SOFTWARE\\Ubisoft\\Launcher\\Installs\\420\\InstallDir\r\n            append: bin\r\n          internal_name: Executable\r\n    offline:\r\n      executables:\r\n        - path:\r\n            relative: FarCry4.exe\r\n          arguments: -playoffline\r\n          evil_updater:\r\n            path:\r\n              relative: Updater.exe\r\n  addons:\r\n    - id: 97\r\n      name: RELATED_GAMENAME_97\r\nlocalizations:\r\n  default:\r\n    GAMENAME: \"Far Cry® 4\"\r\n    RELATED_GAMENAME_97: Valley of the Yetis\r\n  de-DE:\r\n    GAMENAME: Far Cry 4 auf Deutsch\r\n";

    fn record(product: u64, install: u64, settings: &str) -> Vec<u8> {
        let message = [
            write::number(1, product),
            write::number(2, install),
            write::bytes(3, settings.as_bytes()),
        ]
        .concat();
        write::bytes(1, &message)
    }

    #[test]
    fn reads_the_name_and_the_programs_a_game_starts() {
        let cache = parse(&record(420, 420, FAR_CRY_4));
        assert_eq!(
            cache.get("420"),
            Some(&Product {
                name: Some("Far Cry® 4".into()),
                programs: vec!["FarCry4.exe".into()],
            })
        );
    }

    #[test]
    fn names_without_a_localization_stay_as_they_are() {
        let plain = "version: 2.0\nroot:\n  name: \"Assassin's Creed II\"\n  start_game:\n    offline:\n      executables:\n        - path:\n            relative: AssassinsCreedIIGame.exe\n";
        let addon = "version: 2.0\nroot:\n  name: l1\nlocalizations:\n  default:\n    l1: The Hidden Secrets Pack\n";
        let cache = parse(&[record(4, 4, plain), record(116, 0, addon)].concat());
        assert_eq!(cache["4"].name.as_deref(), Some("Assassin's Creed II"));
        assert_eq!(cache["4"].programs, ["AssassinsCreedIIGame.exe"]);
        // An add-on has no install of its own, it is known by its product id.
        assert_eq!(
            cache["116"].name.as_deref(),
            Some("The Hidden Secrets Pack")
        );
        assert!(cache["116"].programs.is_empty());
    }

    #[test]
    fn a_pre_order_sharing_the_install_leaves_the_game_its_record() {
        let pre_order = "version: 2.0\nroot:\n  name: \"Far Cry® 4 PREORDER\"\n";
        for records in [
            [record(420, 420, FAR_CRY_4), record(1471, 420, pre_order)],
            [record(1471, 420, pre_order), record(420, 420, FAR_CRY_4)],
        ] {
            let cache = parse(&records.concat());
            assert_eq!(cache["420"].name.as_deref(), Some("Far Cry® 4"));
            assert_eq!(cache["420"].programs, ["FarCry4.exe"]);
        }
    }

    #[test]
    fn a_damaged_cache_knows_nothing() {
        assert!(parse(b"not a cache").is_empty());
    }

    // Reads this PC's cache and prints a few products, without looking at
    // any game folder: cargo test -- --ignored report_ubisoft --nocapture
    #[test]
    #[ignore = "reads this PC's Ubisoft Connect cache"]
    fn report_ubisoft() {
        let cache = read_cache(Path::new(
            r"C:\Program Files (x86)\Ubisoft\Ubisoft Game Launcher",
        ));
        let named = cache
            .values()
            .filter(|product| product.name.is_some())
            .count();
        let starting = cache
            .values()
            .filter(|product| !product.programs.is_empty())
            .count();
        println!(
            "ubisoft cache: {} products, {named} named, {starting} with programs",
            cache.len()
        );
        for id in ["4", "40", "420", "635", "720"] {
            println!("  {id}: {:?}", cache.get(id));
        }
    }
}
