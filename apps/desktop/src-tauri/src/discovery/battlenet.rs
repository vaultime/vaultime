// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Battle.net's own list of installs, `product.db` in its agent folder. It
//! knows games that have no uninstall entry, like ones moved to another
//! drive and found again.

use std::path::PathBuf;

use super::protobuf::{Value, bytes, fields, number, text};

/// The agent and the launcher itself are installs too.
const NOT_GAMES: &[&str] = &["agent", "battle.net", "bna"];

/// A game Battle.net installed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Install {
    /// Like `fenris` for Diablo IV.
    pub(crate) uid: String,
    pub(crate) folder: String,
}

/// The installed games `product.db` lists.
pub(crate) fn installs() -> Vec<Install> {
    let program_data = std::env::var_os("ProgramData")
        .map_or_else(|| PathBuf::from(r"C:\ProgramData"), PathBuf::from);
    std::fs::read(program_data.join(r"Battle.net\Agent\product.db"))
        .map(|data| parse(&data))
        .unwrap_or_default()
}

/// Each `product_install` (1) with its `uid` (1), `product_code` (2), the
/// install path in its settings (3, 1) and whether it is installed in its
/// cached state (4, 1, 1).
pub(crate) fn parse(data: &[u8]) -> Vec<Install> {
    let Some(database) = fields(data) else {
        return Vec::new();
    };
    database
        .iter()
        .filter_map(|(field, value)| match value {
            Value::Bytes(install) if *field == 1 => fields(install),
            _ => None,
        })
        .filter_map(|install| {
            let uid = text(&install, 1)?;
            let code = text(&install, 2).unwrap_or_default();
            if NOT_GAMES.contains(&uid) || NOT_GAMES.contains(&code) {
                return None;
            }
            let folder = text(&fields(bytes(&install, 3)?)?, 1)?.trim().to_owned();
            let state = fields(bytes(&install, 4)?)?;
            let installed = number(&fields(bytes(&state, 1)?)?, 1) == Some(1);
            (installed && !folder.is_empty()).then(|| Install {
                uid: uid.to_owned(),
                folder,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::protobuf::write;
    use super::*;

    /// A `product_install` as Battle.net writes it.
    fn install(uid: &str, code: &str, folder: &str, installed: bool) -> Vec<u8> {
        let settings = [write::bytes(1, folder.as_bytes()), write::bytes(2, b"us")].concat();
        let base = [write::number(1, u64::from(installed)), write::number(2, 1)].concat();
        let state = write::bytes(1, &base);
        let message = [
            write::bytes(1, uid.as_bytes()),
            write::bytes(2, code.as_bytes()),
            write::bytes(3, &settings),
            write::bytes(4, &state),
        ]
        .concat();
        write::bytes(1, &message)
    }

    #[test]
    fn lists_installed_games_but_not_the_launcher() {
        let data = [
            install("agent", "agent", "C:/ProgramData/Battle.net/Agent", true),
            install(
                "battle.net",
                "bna",
                "C:/Program Files (x86)/Battle.net",
                true,
            ),
            install("fenris", "fenris", "E:/BlizzardLibrary/Diablo IV", true),
            install("pro", "pro", "D:/Games/Overwatch", false),
            install("wow", "wow", "", true),
        ]
        .concat();
        assert_eq!(
            parse(&data),
            [Install {
                uid: "fenris".into(),
                folder: "E:/BlizzardLibrary/Diablo IV".into()
            }]
        );
    }

    #[test]
    fn a_damaged_file_lists_nothing() {
        assert!(parse(b"not a database").is_empty());
        assert!(parse(&[]).is_empty());
    }

    // Reads this PC's product.db and prints what it lists, without looking
    // at the game folders: cargo test -- --ignored report_battlenet --nocapture
    #[test]
    #[ignore = "reads this PC's Battle.net data"]
    fn report_battlenet() {
        println!("battle.net installs {:?}", installs());
    }
}
