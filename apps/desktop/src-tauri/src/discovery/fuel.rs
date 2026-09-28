// SPDX-FileCopyrightText: 2026 Vaultime Contributors
// SPDX-License-Identifier: MIT

//! The `fuel.json` Amazon games keep in their folder, which names the program
//! to start. Both the Amazon Games app and Heroic install such games.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Fuel {
    main: Option<FuelMain>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct FuelMain {
    command: Option<String>,
}

/// The game program named by the `fuel.json` in `folder`, when it exists.
pub(crate) fn program(folder: &Path) -> Option<PathBuf> {
    let json = fs::read_to_string(folder.join("fuel.json")).ok()?;
    Some(folder.join(command(&json)?)).filter(|path| path.is_file())
}

/// The program `fuel.json` starts, relative to the game folder. The file is
/// not always strict JSON, so the `Command` value is also looked up by hand.
fn command(json: &str) -> Option<String> {
    serde_json::from_str::<Fuel>(json)
        .ok()
        .and_then(|fuel| fuel.main?.command)
        .or_else(|| lenient_command(json))
        .filter(|command| !command.is_empty())
}

/// The string after the first `"Command":`, with its escapes undone.
fn lenient_command(json: &str) -> Option<String> {
    const KEY: &str = "\"Command\"";
    let rest = &json[json.find(KEY)? + KEY.len()..];
    let mut chars = rest
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?
        .chars();
    let mut value = String::new();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(value),
            '\\' => value.push(chars.next()?),
            c => value.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_program_from_fuel_json() {
        let json = r#"{"SchemaVersion": "2", "Main": {"Command": "Bin/Game.exe", "Args": []}}"#;
        assert_eq!(command(json).as_deref(), Some("Bin/Game.exe"));
        assert_eq!(command(r#"{"Main": {"Command": ""}}"#), None);
        assert_eq!(command("not json"), None);

        // Comments and trailing commas, which strict JSON does not allow.
        let loose = r#"{
            // Launch settings
            "Main": { "Command": "Bin\\Game.exe", },
        }"#;
        assert_eq!(command(loose).as_deref(), Some(r"Bin\Game.exe"));
    }
}
