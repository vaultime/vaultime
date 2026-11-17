// SPDX-FileCopyrightText: 2026 Dominik Schwimmbeck
// SPDX-License-Identifier: GPL-3.0-or-later

//! Updates a distribution package brings itself. Such a package names what
//! updates it in `PACKAGE_MANAGER_FILE`, and the app then never replaces the
//! files the package manager owns.

use std::fs;
use std::path::Path;

use crate::constants::{PACKAGE_MANAGER_FILE, PACKAGE_MANAGER_NAME_MAX_CHARS};

/// What brings the updates of the program at `program`, like "the AUR",
/// when a package says so. The file sits below the folder above the
/// program's own, `/usr/lib/...` for `/usr/bin/vaultime`.
pub fn managed_by(program: &Path) -> Option<String> {
    let prefix = program.parent()?.parent()?;
    let text = fs::read_to_string(prefix.join(PACKAGE_MANAGER_FILE)).ok()?;
    let name = text.lines().next()?.trim();
    (!name.is_empty() && name.chars().count() <= PACKAGE_MANAGER_NAME_MAX_CHARS)
        .then(|| name.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_package_names_what_brings_its_updates() {
        let prefix =
            std::env::temp_dir().join(format!("vaultime-updates-test-{}", uuid::Uuid::new_v4()));
        let program = prefix.join("bin").join("vaultime");
        assert_eq!(managed_by(&program), None);

        let marker = prefix.join(PACKAGE_MANAGER_FILE);
        fs::create_dir_all(marker.parent().unwrap()).unwrap();
        fs::write(&marker, "the AUR\n").unwrap();
        assert_eq!(managed_by(&program).as_deref(), Some("the AUR"));

        fs::write(&marker, "  \n").unwrap();
        assert_eq!(managed_by(&program), None);
        fs::write(&marker, "x".repeat(PACKAGE_MANAGER_NAME_MAX_CHARS + 1)).unwrap();
        assert_eq!(managed_by(&program), None);
        fs::remove_dir_all(prefix).unwrap();
    }
}
