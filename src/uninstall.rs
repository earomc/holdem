//! What `holdem uninstall` removes: finding how this copy was installed (the
//! receipt the release installers leave), and listing what goes.
//!
//! Nothing here deletes anything; the command carries out the [`Plan`].

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The release installers' record of an install (only the fields used).
#[derive(Debug, Deserialize)]
struct Receipt {
    binaries: Vec<String>,
    install_prefix: String,
    #[serde(default)]
    install_layout: String,
    #[serde(default = "yes")]
    modify_path: bool,
    #[serde(default)]
    version: String,
}

fn yes() -> bool {
    true
}

/// A copy of holdem put in place by a release installer.
#[derive(Clone, Debug, PartialEq)]
pub struct Install {
    /// The program.
    pub exe: PathBuf,
    /// The folder it is in.
    pub bin_dir: PathBuf,
    /// The installer's receipt.
    pub receipt: PathBuf,
    /// The installer added `bin_dir` to the PATH.
    pub modify_path: bool,
    pub version: String,
}

/// Everything `holdem uninstall` removes, and what it keeps.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// Files to delete (the receipt, and with purge your settings and statistics).
    pub files: Vec<PathBuf>,
    /// Folders to remove afterwards, if nothing is left in them.
    pub dirs: Vec<PathBuf>,
    /// Your settings and statistics, kept.
    pub kept: Vec<PathBuf>,
    /// A folder to take off the PATH: the program's, once nothing else is in it.
    pub path_entry: Option<PathBuf>,
}

/// The files your settings and statistics are kept in.
const DATA_FILES: [&str; 2] = [crate::config::FILE_NAME, crate::history::FILE_NAME];

/// Where the installers of holdem and holdem-server leave their receipts:
/// `$XDG_CONFIG_HOME/<app>`, else `%LOCALAPPDATA%\<app>` on Windows and
/// `~/.config/<app>` elsewhere.
pub fn receipt_files() -> Vec<PathBuf> {
    let var = |name: &str| {
        std::env::var_os(name)
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
    };
    let base = var("XDG_CONFIG_HOME").or_else(|| {
        if cfg!(windows) {
            var("LOCALAPPDATA")
        } else {
            var("HOME").map(|h| h.join(".config"))
        }
    });
    let Some(base) = base else {
        return Vec::new();
    };
    ["holdem", "holdem-server"]
        .iter()
        .map(|app| base.join(app).join(format!("{app}-receipt.json")))
        .collect()
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// The install `exe` belongs to, from the first of `receipts` that names it.
/// `None` for a copy no installer put there (an archive, a build).
pub fn find(exe: &Path, receipts: &[PathBuf]) -> Option<Install> {
    for path in receipts {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        let Ok(receipt) = serde_json::from_str::<Receipt>(&text) else {
            continue;
        };
        let prefix = PathBuf::from(&receipt.install_prefix);
        let bin_dir = if receipt.install_layout == "flat" {
            prefix
        } else {
            prefix.join("bin")
        };
        for binary in &receipt.binaries {
            let candidate = bin_dir.join(binary);
            if same_file(&candidate, exe) {
                return Some(Install {
                    exe: candidate,
                    bin_dir,
                    receipt: path.clone(),
                    modify_path: receipt.modify_path,
                    version: receipt.version,
                });
            }
        }
    }
    None
}

/// Your settings and statistics in `settings_dir` that exist.
pub fn data_files(settings_dir: &Path) -> Vec<PathBuf> {
    DATA_FILES
        .iter()
        .map(|name| settings_dir.join(name))
        .filter(|p| p.is_file())
        .collect()
}

/// What uninstalling `install` removes. With `purge`, your settings and
/// statistics in `settings_dir` go too; only those files, never the folder's
/// other contents. (On Linux the receipt and the settings share a folder, so
/// folders are only removed once empty.)
pub fn plan(install: &Install, settings_dir: Option<&Path>, purge: bool) -> Plan {
    let mut plan = Plan {
        files: vec![install.receipt.clone()],
        ..Plan::default()
    };
    if let Some(dir) = install.receipt.parent() {
        plan.dirs.push(dir.to_path_buf());
    }
    if let Some(dir) = settings_dir {
        let data = data_files(dir);
        if purge {
            plan.files.extend(data);
            if !plan.dirs.iter().any(|d| d == dir) {
                plan.dirs.push(dir.to_path_buf());
            }
        } else {
            plan.kept = data;
        }
    }
    let alone = std::fs::read_dir(&install.bin_dir).is_ok_and(|entries| {
        entries
            .filter_map(Result::ok)
            .all(|e| same_file(&e.path(), &install.exe))
    });
    if install.modify_path && alone {
        plan.path_entry = Some(install.bin_dir.clone());
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fake install in a temp folder: `.cargo/bin/holdem.exe`, its receipt
    /// and a settings folder with both data files.
    struct Fake {
        root: PathBuf,
        exe: PathBuf,
        receipt: PathBuf,
        settings: PathBuf,
    }

    fn fake(name: &str, layout: &str) -> Fake {
        let root =
            std::env::temp_dir().join(format!("holdem-uninstall-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let cargo = root.join(".cargo");
        let bin = cargo.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("holdem.exe");
        std::fs::write(&exe, "program").unwrap();
        let receipt = root
            .join("local")
            .join("holdem")
            .join("holdem-receipt.json");
        std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        let text = serde_json::json!({
            "binaries": ["holdem.exe"],
            "install_layout": layout,
            "install_prefix": cargo.to_string_lossy(),
            "modify_path": true,
            "version": "0.9.0",
        });
        std::fs::write(&receipt, text.to_string()).unwrap();
        let settings = root.join("roaming").join("holdem");
        std::fs::create_dir_all(&settings).unwrap();
        for f in DATA_FILES {
            std::fs::write(settings.join(f), "x").unwrap();
        }
        Fake {
            root,
            exe,
            receipt,
            settings,
        }
    }

    #[test]
    fn the_receipt_that_names_the_program_is_found() {
        let f = fake("find", "cargo-home");
        let other = f.root.join("other-receipt.json");
        std::fs::write(
            &other,
            r#"{"binaries":["holdem.exe"],"install_prefix":"/somewhere/else"}"#,
        )
        .unwrap();
        let broken = f.root.join("broken-receipt.json");
        std::fs::write(&broken, "not json").unwrap();
        let receipts = [other.clone(), broken, f.receipt.clone()];
        let install = find(&f.exe, &receipts).expect("found");
        assert_eq!(install.receipt, f.receipt);
        assert_eq!(install.version, "0.9.0");
        assert!(install.modify_path);
        assert!(find(&f.exe, &[other]).is_none(), "another prefix");
        let build = f.root.join("target").join("holdem.exe");
        std::fs::create_dir_all(build.parent().unwrap()).unwrap();
        std::fs::write(&build, "program").unwrap();
        assert!(
            find(&build, std::slice::from_ref(&f.receipt)).is_none(),
            "not installed"
        );
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn settings_and_statistics_go_only_with_purge() {
        let f = fake("purge", "cargo-home");
        let install = find(&f.exe, std::slice::from_ref(&f.receipt)).unwrap();
        let keep = plan(&install, Some(&f.settings), false);
        assert_eq!(keep.files, vec![f.receipt.clone()]);
        assert_eq!(keep.kept.len(), 2);
        assert!(!keep.dirs.contains(&f.settings));
        let purge = plan(&install, Some(&f.settings), true);
        assert_eq!(purge.files.len(), 3);
        assert!(purge.files.contains(&f.settings.join("holdem.toml")));
        assert!(purge.files.contains(&f.settings.join("stats.toml")));
        assert!(purge.kept.is_empty());
        assert!(purge.dirs.contains(&f.settings));
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn the_path_entry_goes_only_when_the_folder_is_otherwise_empty() {
        let f = fake("path", "cargo-home");
        let install = find(&f.exe, std::slice::from_ref(&f.receipt)).unwrap();
        assert_eq!(
            plan(&install, None, false).path_entry,
            Some(install.bin_dir.clone())
        );
        std::fs::write(install.bin_dir.join("cargo.exe"), "rust").unwrap();
        assert_eq!(
            plan(&install, None, false).path_entry,
            None,
            "Rust is there too"
        );
        let no_path = Install {
            modify_path: false,
            ..install
        };
        std::fs::remove_file(no_path.bin_dir.join("cargo.exe")).unwrap();
        assert_eq!(plan(&no_path, None, false).path_entry, None);
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn a_shared_receipt_and_settings_folder_is_only_emptied() {
        // On Linux both live in ~/.config/holdem.
        let f = fake("shared", "cargo-home");
        let install = find(&f.exe, std::slice::from_ref(&f.receipt)).unwrap();
        let shared = f.receipt.parent().unwrap().to_path_buf();
        for name in DATA_FILES {
            std::fs::write(shared.join(name), "x").unwrap();
        }
        let keep = plan(&install, Some(&shared), false);
        assert_eq!(keep.files, vec![f.receipt.clone()]);
        assert_eq!(keep.kept.len(), 2);
        assert_eq!(
            keep.dirs,
            vec![shared.clone()],
            "listed once, removed only if empty"
        );
        let purge = plan(&install, Some(&shared), true);
        assert_eq!(purge.dirs, vec![shared]);
        let _ = std::fs::remove_dir_all(&f.root);
    }

    #[test]
    fn a_flat_layout_has_no_bin_folder() {
        let f = fake("flat", "flat");
        let flat_exe = f.root.join(".cargo").join("holdem.exe");
        std::fs::write(&flat_exe, "program").unwrap();
        let install = find(&flat_exe, std::slice::from_ref(&f.receipt)).expect("found");
        assert_eq!(install.bin_dir, f.root.join(".cargo"));
        let _ = std::fs::remove_dir_all(&f.root);
    }
}
