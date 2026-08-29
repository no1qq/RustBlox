use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use crate::error::{Context, Error, Result};
use crate::util::fs;

use super::install::Installation;

pub const FONT_STEM: &str = "CustomFont";

const ABSENT: &str = ".rustblox-absent";
const MAX_DEPTH: u32 = 12;

fn fonts_dir(root: &Path) -> PathBuf {
    root.join("content").join("fonts")
}

fn families_dir(root: &Path) -> PathBuf {
    fonts_dir(root).join("families")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub relative: PathBuf,
    pub bytes: u64,
}

impl Entry {
    pub fn display(&self) -> String {
        self.relative.to_string_lossy().replace('\\', "/")
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Inventory {
    pub root: PathBuf,
    pub files: Vec<Entry>,
    pub bytes: u64,
}

impl Inventory {
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn font(&self) -> Option<&Entry> {
        self.files
            .iter()
            .find(|entry| is_custom_font(&entry.relative))
    }

    pub fn holds(&self, relative: &Path) -> bool {
        self.files.iter().any(|entry| entry.relative == relative)
    }
}

pub fn scan(root: &Path) -> Inventory {
    let mut inventory = Inventory {
        root: root.to_path_buf(),
        ..Inventory::default()
    };
    walk(root, Path::new(""), 0, &mut inventory);
    inventory.files.sort_by(|a, b| a.relative.cmp(&b.relative));
    inventory
}

fn walk(dir: &Path, prefix: &Path, depth: u32, inventory: &mut Inventory) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with('.') || name.eq_ignore_ascii_case("desktop.ini") {
            continue;
        }

        let relative = prefix.join(name);
        let path = entry.path();
        if path.is_dir() {
            walk(&path, &relative, depth + 1, inventory);
        } else if let Ok(meta) = entry.metadata() {
            inventory.bytes += meta.len();
            inventory.files.push(Entry {
                relative,
                bytes: meta.len(),
            });
        }
    }
}

fn is_safe(relative: &Path) -> bool {
    relative
        .components()
        .all(|part| matches!(part, Component::Normal(name) if !name.to_string_lossy().is_empty()))
}

pub fn is_custom_font(relative: &Path) -> bool {
    let Ok(rest) = relative.strip_prefix(fonts_dir(Path::new(""))) else {
        return false;
    };
    rest.file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem == FONT_STEM)
        && rest
            .parent()
            .is_some_and(|parent| parent.as_os_str().is_empty())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub applied: usize,
    pub unchanged: usize,
    pub restored: usize,
    pub refused: Vec<String>,
}

impl Report {
    pub fn summary(&self) -> String {
        match (self.applied, self.restored) {
            (0, 0) => "no mods to apply".to_owned(),
            (applied, 0) => format!("laid {applied} mod files over the client"),
            (0, restored) => format!("put {restored} original files back"),
            (applied, restored) => {
                format!("laid {applied} mod files over the client and put {restored} back")
            }
        }
    }
}

pub fn apply(version_dir: &Path, mods_root: &Path, originals_root: &Path) -> Result<Report> {
    let inventory = scan(mods_root);
    let mut report = Report::default();

    for entry in &inventory.files {
        if !is_safe(&entry.relative) {
            report.refused.push(entry.display());
            continue;
        }

        let target = version_dir.join(&entry.relative);
        let source = mods_root.join(&entry.relative);
        let original = originals_root.join(&entry.relative);
        let marker = marker_for(&original);

        if !original.is_file() && !marker.is_file() {
            if target.is_file() {
                copy(&target, &original)?;
            } else {
                fs::write_atomic(&marker, b"")?;
            }
        }

        if same_bytes(&source, &target) {
            report.unchanged += 1;
        } else {
            copy(&source, &target)?;
            report.applied += 1;
        }
    }

    report.restored = put_back(version_dir, originals_root, Some(&inventory))?;
    Ok(report)
}

pub fn restore_all(version_dir: &Path, originals_root: &Path) -> Result<usize> {
    put_back(version_dir, originals_root, None)
}

fn put_back(
    version_dir: &Path,
    originals_root: &Path,
    keeping: Option<&Inventory>,
) -> Result<usize> {
    let saved = scan(originals_root);
    let mut restored = 0;

    for entry in &saved.files {
        let (relative, was_absent) = match entry.relative.to_string_lossy().strip_suffix(ABSENT) {
            Some(trimmed) => (PathBuf::from(trimmed), true),
            None => (entry.relative.clone(), false),
        };

        if keeping.is_some_and(|inventory| inventory.holds(&relative)) {
            continue;
        }

        let target = version_dir.join(&relative);
        let original = originals_root.join(&entry.relative);

        if was_absent {
            let _ = std::fs::remove_file(&target);
        } else {
            copy(&original, &target)?;
        }
        let _ = std::fs::remove_file(&original);
        restored += 1;
    }

    prune_empty(originals_root);
    Ok(restored)
}

fn marker_for(original: &Path) -> PathBuf {
    let mut name = original.as_os_str().to_os_string();
    name.push(ABSENT);
    PathBuf::from(name)
}

fn copy(from: &Path, to: &Path) -> Result<()> {
    let bytes = std::fs::read(from).ctx_path("could not read", from)?;
    fs::write_atomic(to, &bytes)
}

fn same_bytes(left: &Path, right: &Path) -> bool {
    let (Ok(one), Ok(two)) = (std::fs::metadata(left), std::fs::metadata(right)) else {
        return false;
    };
    if one.len() != two.len() {
        return false;
    }
    match (std::fs::read(left), std::fs::read(right)) {
        (Ok(one), Ok(two)) => one == two,
        _ => false,
    }
}

fn prune_empty(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            prune_empty(&path);
            let _ = std::fs::remove_dir(&path);
        }
    }
}

pub fn forget_other_versions(originals_root: &Path, keeping: &str) {
    let Ok(entries) = std::fs::read_dir(originals_root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.file_name().is_some_and(|name| name != keeping) {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

pub fn font_target(source: &Path) -> PathBuf {
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| extension.eq_ignore_ascii_case("otf"))
        .unwrap_or("ttf")
        .to_ascii_lowercase();
    fonts_dir(Path::new("")).join(format!("{FONT_STEM}.{extension}"))
}

pub fn install_font(mods_root: &Path, install: &Installation, source: &Path) -> Result<usize> {
    let extension = source
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension != "ttf" && extension != "otf" {
        return Err(Error::invalid("a font has to be a .ttf or an .otf file"));
    }

    remove_font(mods_root)?;

    let relative = font_target(source);
    copy(source, &mods_root.join(&relative))?;
    let asset = format!(
        "rbxasset://fonts/{}",
        relative
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(FONT_STEM)
    );

    let families = families_dir(&install.version_dir);
    let Ok(entries) = std::fs::read_dir(&families) else {
        return Err(Error::invalid(
            "the installed client has no font families to rewrite",
        ));
    };

    let mut written = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() || path.extension().is_none_or(|ext| ext != "json") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(mut value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };

        point_at(&mut value, &asset);
        let mut body = serde_json::to_string_pretty(&value).unwrap_or_default();
        body.push('\n');
        fs::write_atomic(&families_dir(mods_root).join(name), body.as_bytes())?;
        written += 1;
    }

    if written == 0 {
        remove_font(mods_root)?;
        return Err(Error::invalid("no font families could be read"));
    }

    Ok(written)
}

fn point_at(value: &mut Value, asset: &str) {
    let Some(faces) = value.get_mut("faces").and_then(Value::as_array_mut) else {
        return;
    };
    for face in faces {
        if let Some(id) = face.get_mut("assetId") {
            *id = Value::String(asset.to_owned());
        }
    }
}

pub fn remove_font(mods_root: &Path) -> Result<()> {
    let fonts = fonts_dir(mods_root);
    let _ = std::fs::remove_dir_all(families_dir(mods_root));

    if let Ok(entries) = std::fs::read_dir(&fonts) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem == FONT_STEM)
            {
                let _ = std::fs::remove_file(&path);
            }
        }
    }

    prune_empty(mods_root);
    Ok(())
}

const CLASSIC_OOF_BYTES: &[u8] = include_bytes!("../../assets/sounds/ouch.ogg");

pub fn apply_death_sound_preset(
    mods_root: &Path,
    preset: crate::config::DeathSoundPreset,
) -> Result<()> {
    let target = mods_root.join("content").join("sounds").join("ouch.ogg");
    match preset {
        crate::config::DeathSoundPreset::ClassicOof => {
            if let Some(parent) = target.parent() {
                fs::ensure_dir(parent)?;
            }
            fs::write_atomic(&target, CLASSIC_OOF_BYTES)?;
        }
        crate::config::DeathSoundPreset::Default => {
            if target.is_file() {
                let _ = std::fs::remove_file(&target);
                prune_empty(mods_root);
            }
        }
        crate::config::DeathSoundPreset::Custom => {}
    }
    Ok(())
}

pub fn install_custom_death_sound(mods_root: &Path, source: &Path) -> Result<()> {
    let target = mods_root.join("content").join("sounds").join("ouch.ogg");
    if let Some(parent) = target.parent() {
        fs::ensure_dir(parent)?;
    }
    copy(source, &target)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, body: &str) {
        fs::write_atomic(path, body.as_bytes()).unwrap();
    }

    fn client(dir: &Path) -> PathBuf {
        let version = dir.join("version-abc");
        write(&version.join("content").join("hello.txt"), "original");
        write(&version.join("keep.txt"), "kept");
        version
    }

    #[test]
    fn an_empty_folder_holds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let inventory = scan(&dir.path().join("mods"));
        assert!(inventory.is_empty());
        assert_eq!(inventory.bytes, 0);
    }

    #[test]
    fn the_folder_is_read_as_paths_relative_to_the_client() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        write(&mods.join("content").join("hello.txt"), "mine");
        write(&mods.join("top.txt"), "also mine");

        let inventory = scan(&mods);
        let names: Vec<String> = inventory.files.iter().map(Entry::display).collect();

        assert_eq!(names, vec!["content/hello.txt", "top.txt"]);
        assert_eq!(inventory.bytes, 13);
    }

    #[test]
    fn hidden_files_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        let mods = dir.path().join("mods");
        write(&mods.join(".gitkeep"), "");
        write(&mods.join("desktop.ini"), "");
        write(&mods.join("real.txt"), "yes");

        assert_eq!(scan(&mods).files.len(), 1);
    }

    #[test]
    fn applying_replaces_the_client_file_and_keeps_the_original() {
        let dir = tempfile::tempdir().unwrap();
        let version = client(dir.path());
        let mods = dir.path().join("mods");
        let originals = dir.path().join("originals");
        write(&mods.join("content").join("hello.txt"), "mine");

        let report = apply(&version, &mods, &originals).unwrap();

        assert_eq!(report.applied, 1);
        assert_eq!(
            std::fs::read_to_string(version.join("content").join("hello.txt")).unwrap(),
            "mine"
        );
        assert_eq!(
            std::fs::read_to_string(originals.join("content").join("hello.txt")).unwrap(),
            "original"
        );
    }

    #[test]
    fn applying_twice_changes_nothing_the_second_time() {
        let dir = tempfile::tempdir().unwrap();
        let version = client(dir.path());
        let mods = dir.path().join("mods");
        let originals = dir.path().join("originals");
        write(&mods.join("content").join("hello.txt"), "mine");

        apply(&version, &mods, &originals).unwrap();
        let again = apply(&version, &mods, &originals).unwrap();

        assert_eq!(again.applied, 0);
        assert_eq!(again.unchanged, 1);
        assert_eq!(
            std::fs::read_to_string(originals.join("content").join("hello.txt")).unwrap(),
            "original"
        );
    }

    #[test]
    fn taking_a_mod_out_of_the_folder_puts_the_original_back() {
        let dir = tempfile::tempdir().unwrap();
        let version = client(dir.path());
        let mods = dir.path().join("mods");
        let originals = dir.path().join("originals");
        write(&mods.join("content").join("hello.txt"), "mine");

        apply(&version, &mods, &originals).unwrap();
        std::fs::remove_file(mods.join("content").join("hello.txt")).unwrap();
        let report = apply(&version, &mods, &originals).unwrap();

        assert_eq!(report.restored, 1);
        assert_eq!(
            std::fs::read_to_string(version.join("content").join("hello.txt")).unwrap(),
            "original"
        );
        assert!(!originals.join("content").join("hello.txt").exists());
    }

    #[test]
    fn a_mod_the_client_never_had_is_deleted_again() {
        let dir = tempfile::tempdir().unwrap();
        let version = client(dir.path());
        let mods = dir.path().join("mods");
        let originals = dir.path().join("originals");
        write(&mods.join("content").join("brand-new.txt"), "mine");

        apply(&version, &mods, &originals).unwrap();
        assert!(version.join("content").join("brand-new.txt").is_file());

        std::fs::remove_file(mods.join("content").join("brand-new.txt")).unwrap();
        let report = apply(&version, &mods, &originals).unwrap();

        assert_eq!(report.restored, 1);
        assert!(!version.join("content").join("brand-new.txt").exists());
    }

    #[test]
    fn turning_mods_off_puts_everything_back() {
        let dir = tempfile::tempdir().unwrap();
        let version = client(dir.path());
        let mods = dir.path().join("mods");
        let originals = dir.path().join("originals");
        write(&mods.join("content").join("hello.txt"), "mine");
        write(&mods.join("content").join("brand-new.txt"), "mine");

        apply(&version, &mods, &originals).unwrap();
        let restored = restore_all(&version, &originals).unwrap();

        assert_eq!(restored, 2);
        assert_eq!(
            std::fs::read_to_string(version.join("content").join("hello.txt")).unwrap(),
            "original"
        );
        assert!(!version.join("content").join("brand-new.txt").exists());
        assert!(scan(&originals).is_empty());
    }

    #[test]
    fn files_the_mods_folder_never_touched_are_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let version = client(dir.path());
        let mods = dir.path().join("mods");
        let originals = dir.path().join("originals");
        write(&mods.join("content").join("hello.txt"), "mine");

        apply(&version, &mods, &originals).unwrap();
        restore_all(&version, &originals).unwrap();

        assert_eq!(
            std::fs::read_to_string(version.join("keep.txt")).unwrap(),
            "kept"
        );
    }

    #[test]
    fn the_saved_originals_of_older_versions_are_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("originals");
        write(&root.join("version-old").join("a.txt"), "a");
        write(&root.join("version-new").join("b.txt"), "b");

        forget_other_versions(&root, "version-new");

        assert!(!root.join("version-old").exists());
        assert!(root.join("version-new").join("b.txt").is_file());
    }

    #[test]
    fn a_custom_font_is_recognised_wherever_it_is_not() {
        assert!(is_custom_font(
            &Path::new("content").join("fonts").join("CustomFont.ttf")
        ));
        assert!(is_custom_font(
            &Path::new("content").join("fonts").join("CustomFont.otf")
        ));
        assert!(!is_custom_font(
            &Path::new("content").join("fonts").join("Other.ttf")
        ));
        assert!(!is_custom_font(
            &Path::new("content")
                .join("fonts")
                .join("families")
                .join("CustomFont.json")
        ));
    }

    #[test]
    fn a_font_family_is_pointed_at_the_custom_file() {
        let mut value = serde_json::json!({
            "name": "Source Sans Pro",
            "faces": [
                {"name": "Regular", "assetId": "rbxasset://fonts/SourceSansPro-Regular.ttf"},
                {"name": "Bold", "assetId": "rbxassetid://8075251923"}
            ]
        });

        point_at(&mut value, "rbxasset://fonts/CustomFont.ttf");

        for face in value["faces"].as_array().unwrap() {
            assert_eq!(face["assetId"], "rbxasset://fonts/CustomFont.ttf");
        }
        assert_eq!(value["name"], "Source Sans Pro");
        assert_eq!(value["faces"][0]["name"], "Regular");
    }

    #[test]
    fn the_font_keeps_the_extension_it_came_with() {
        assert!(font_target(Path::new("C:/x/Comic.OTF"))
            .to_string_lossy()
            .ends_with("CustomFont.otf"));
        assert!(font_target(Path::new("C:/x/Comic.ttf"))
            .to_string_lossy()
            .ends_with("CustomFont.ttf"));
    }
}
