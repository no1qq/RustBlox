use std::path::{Path, PathBuf};

use crate::config::Paths;
use crate::platform;
use crate::roblox::install::format_size;
use crate::roblox::uri::{SCHEME_DEEPLINK, SCHEME_PLAYER};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Plan {
    pub remove_settings: bool,
    pub remove_executable: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub removed: Vec<String>,
    pub freed: u64,
    pub problems: Vec<String>,
}

impl Report {
    pub fn summary(&self) -> String {
        if self.removed.is_empty() {
            return "nothing was left to remove".into();
        }
        format!(
            "{} removed, {} freed",
            self.removed.join(" and "),
            format_size(self.freed)
        )
    }
}

fn size_of(path: &Path) -> u64 {
    crate::roblox::install::directory_size(path, 0).unwrap_or(0)
}

fn drop_directory(path: &Path, label: &str, keep: Option<&Path>, report: &mut Report) {
    if !path.exists() {
        return;
    }

    let spare = keep.filter(|kept| kept.starts_with(path) && *kept != path);
    let size = size_of(path).saturating_sub(spare.map(size_of).unwrap_or(0));

    let outcome = match spare {
        None => std::fs::remove_dir_all(path),
        Some(spare) => remove_around(path, spare),
    };

    match outcome {
        Ok(()) => {
            report.removed.push(label.to_owned());
            report.freed = report.freed.saturating_add(size);
        }
        Err(err) => report
            .problems
            .push(format!("{} could not be removed: {err}", path.display())),
    }
}

fn remove_around(root: &Path, spare: &Path) -> std::io::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let path = entry?.path();
        if spare.starts_with(&path) {
            if path != spare {
                remove_around(&path, spare)?;
            }
            continue;
        }
        if path.is_dir() {
            std::fs::remove_dir_all(&path)?;
        } else {
            std::fs::remove_file(&path)?;
        }
    }
    Ok(())
}

pub fn targets(paths: &Paths, plan: Plan) -> Vec<PathBuf> {
    let mut list = vec![paths.data_dir().to_path_buf()];
    if plan.remove_settings {
        list.push(paths.config_dir().to_path_buf());
    }
    list
}

pub fn run(paths: &Paths, plan: Plan, exe: Option<&Path>) -> Report {
    let mut report = Report::default();

    for scheme in [SCHEME_PLAYER, SCHEME_DEEPLINK] {
        if let Ok(registration) = platform::protocol::inspect(scheme) {
            if registration.owner == platform::SchemeOwner::Ours {
                if let Err(err) = platform::protocol::restore(scheme) {
                    report
                        .problems
                        .push(format!("the {scheme} handler was left in place: {err}"));
                }
            }
        }
    }

    let shortcuts = crate::shortcuts::remove_all();
    if !shortcuts.is_empty() {
        report.removed.push(match shortcuts.len() {
            1 => "1 shortcut".to_owned(),
            count => format!("{count} shortcuts"),
        });
    }

    let keep = if plan.remove_settings {
        drop_directory(paths.config_dir(), "settings", None, &mut report);
        None
    } else {
        Some(paths.config_dir())
    };

    drop_directory(
        paths.data_dir(),
        "Roblox copies, logs and state",
        keep,
        &mut report,
    );

    if plan.remove_executable {
        if let Some(exe) = exe {
            if let Err(err) = schedule_self_delete(exe) {
                report.problems.push(err);
            } else {
                report.removed.push("the RustBlox program".into());
            }
        }
    }

    report
}

#[cfg(windows)]
fn schedule_self_delete(exe: &Path) -> std::result::Result<(), String> {
    use std::os::windows::process::CommandExt;

    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    std::process::Command::new("cmd")
        .args([
            "/C",
            "ping",
            "127.0.0.1",
            "-n",
            "4",
            ">nul",
            "&",
            "del",
            "/f",
            "/q",
        ])
        .arg(exe)
        .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|err| format!("the program file could not be scheduled for removal: {err}"))
}

#[cfg(not(windows))]
fn schedule_self_delete(_exe: &Path) -> std::result::Result<(), String> {
    Err("removing the program file is only supported on Windows".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_in(root: &Path) -> Paths {
        Paths::rooted(root)
    }

    fn seed(paths: &Paths) {
        std::fs::create_dir_all(paths.data_dir().join("Versions")).unwrap();
        std::fs::write(paths.data_dir().join("Versions").join("a.bin"), [0u8; 64]).unwrap();
        std::fs::create_dir_all(paths.config_dir()).unwrap();
        std::fs::write(paths.settings_file(), b"{}").unwrap();
    }

    #[test]
    fn keeping_settings_leaves_the_config_folder_behind() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        seed(&paths);

        let report = run(
            &paths,
            Plan {
                remove_settings: false,
                remove_executable: false,
            },
            None,
        );

        assert!(paths.settings_file().is_file());
        assert!(!paths.data_dir().join("Versions").exists());
        assert!(report.problems.is_empty());
    }

    #[test]
    fn removing_settings_takes_the_config_folder_too() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        seed(&paths);

        let report = run(
            &paths,
            Plan {
                remove_settings: true,
                remove_executable: false,
            },
            None,
        );

        assert!(!paths.settings_file().exists());
        assert!(report.removed.iter().any(|entry| entry == "settings"));
    }

    #[test]
    fn what_gets_removed_is_listed_before_anything_happens() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());

        let keep = targets(
            &paths,
            Plan {
                remove_settings: false,
                remove_executable: false,
            },
        );
        let wipe = targets(
            &paths,
            Plan {
                remove_settings: true,
                remove_executable: false,
            },
        );

        assert_eq!(keep, vec![paths.data_dir().to_path_buf()]);
        assert_eq!(
            wipe,
            vec![
                paths.data_dir().to_path_buf(),
                paths.config_dir().to_path_buf()
            ]
        );
    }

    #[test]
    fn a_portable_layout_keeps_settings_that_sit_inside_the_data_folder() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        seed(&paths);

        assert!(
            paths.config_dir().starts_with(paths.data_dir()),
            "this test only means something when config sits inside data"
        );

        run(
            &paths,
            Plan {
                remove_settings: false,
                remove_executable: false,
            },
            None,
        );

        assert!(paths.settings_file().is_file());
        assert!(!paths.data_dir().join("Versions").exists());
    }

    #[test]
    fn uninstalling_twice_is_not_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        seed(&paths);

        let plan = Plan {
            remove_settings: true,
            remove_executable: false,
        };
        let first = run(&paths, plan, None);
        let second = run(&paths, plan, None);

        assert!(!first.removed.is_empty());
        assert!(second.removed.is_empty());
        assert!(second.problems.is_empty());
        assert_eq!(second.summary(), "nothing was left to remove");
    }

    #[test]
    fn the_summary_reports_what_was_freed() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        seed(&paths);

        let report = run(
            &paths,
            Plan {
                remove_settings: false,
                remove_executable: false,
            },
            None,
        );

        assert!(report.freed >= 64);
        assert!(report.summary().contains("freed"));
    }
}
