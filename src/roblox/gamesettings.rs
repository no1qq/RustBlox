use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::util::fs;

pub const BACKUPS_KEPT: usize = 10;

const FILE_PREFIX: &str = "GlobalBasicSettings_";
const FILE_SUFFIX: &str = ".xml";
const FALLBACK_FILE: &str = "GlobalBasicSettings_13.xml";
const PROPERTIES_END: &str = "</Properties>";

pub const FRAMERATE_CAP: &str = "FramerateCap";
pub const QUALITY: &str = "SavedQualityLevel";
pub const PERFORMANCE_STATS: &str = "PerformanceStatsVisible";
pub const TRANSPARENCY: &str = "PreferredTransparency";
pub const REDUCED_MOTION: &str = "ReducedMotion";
pub const TEXT_SIZE: &str = "PreferredTextSize";
pub const MOUSE_SENSITIVITY: &str = "MouseSensitivity";
pub const MOUSE_FIRST_PERSON: &str = "MouseSensitivityFirstPerson";
pub const MOUSE_THIRD_PERSON: &str = "MouseSensitivityThirdPerson";
pub const VR_ENABLED: &str = "VREnabled";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Bool,
    Int,
    Float,
    Token,
    Vector2,
}

impl Kind {
    fn tag(self) -> &'static str {
        match self {
            Kind::Bool => "bool",
            Kind::Int => "int",
            Kind::Float => "float",
            Kind::Token => "token",
            Kind::Vector2 => "Vector2",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Change {
    pub name: &'static str,
    pub kind: Kind,
    pub value: String,
}

impl Change {
    pub fn flag(name: &'static str, value: bool) -> Self {
        Self {
            name,
            kind: Kind::Bool,
            value: if value { "true".into() } else { "false".into() },
        }
    }

    pub fn int(name: &'static str, value: i64) -> Self {
        Self {
            name,
            kind: Kind::Int,
            value: value.to_string(),
        }
    }

    pub fn token(name: &'static str, value: i64) -> Self {
        Self {
            name,
            kind: Kind::Token,
            value: value.to_string(),
        }
    }

    pub fn float(name: &'static str, value: f32) -> Self {
        Self {
            name,
            kind: Kind::Float,
            value: render_float(value),
        }
    }

    pub fn point(name: &'static str, value: f32) -> Self {
        Self {
            name,
            kind: Kind::Vector2,
            value: render_float(value),
        }
    }
}

fn render_float(value: f32) -> String {
    let text = format!("{value}");
    if text.chars().all(|c| c.is_ascii_digit() || c == '-') {
        format!("{text}.0")
    } else {
        text
    }
}

pub fn quality_label(value: u8) -> String {
    match value {
        0 => "Automatic".to_owned(),
        other => format!("Level {other}"),
    }
}

pub fn text_size_label(value: u8) -> &'static str {
    match value {
        2 => "Large",
        3 => "Larger",
        4 => "Largest",
        _ => "Default",
    }
}

pub fn settings_file() -> Option<PathBuf> {
    let dir = super::local_dir()?;
    Some(newest_in(&dir).unwrap_or_else(|| dir.join(FALLBACK_FILE)))
}

fn newest_in(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let name = path.file_name()?.to_str()?;
            let digits = name.strip_prefix(FILE_PREFIX)?.strip_suffix(FILE_SUFFIX)?;
            Some((digits.parse::<u32>().ok()?, path))
        })
        .max_by_key(|(version, _)| *version)
        .map(|(_, path)| path)
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Snapshot {
    pub path: Option<PathBuf>,
    pub found: bool,
    pub locked: bool,
    pub values: BTreeMap<String, String>,
}

impl Snapshot {
    pub fn read() -> Self {
        let Some(path) = settings_file() else {
            return Self::default();
        };
        Self::read_from(&path)
    }

    pub fn read_from(path: &Path) -> Self {
        let text = fs::read_to_string_if_exists(path).ok().flatten();
        Self {
            path: Some(path.to_path_buf()),
            found: text.is_some(),
            locked: fs::is_read_only(path),
            values: text.as_deref().map(scan).unwrap_or_default(),
        }
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn int(&self, name: &str) -> Option<i64> {
        self.text(name)?.trim().parse().ok()
    }

    pub fn float(&self, name: &str) -> Option<f32> {
        self.text(name)?.trim().parse().ok()
    }

    pub fn flag(&self, name: &str) -> Option<bool> {
        match self.text(name)?.trim() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }
}

pub fn scan(text: &str) -> BTreeMap<String, String> {
    let mut found = BTreeMap::new();
    let mut rest = text;

    while let Some(at) = rest.find("name=\"") {
        let after = at + "name=\"".len();
        let Some(quote) = rest[after..].find('"') else {
            break;
        };
        let name = &rest[after..after + quote];
        let start = after + quote + 1;

        if let Some(body) = rest[start..].strip_prefix('>') {
            if let Some(tag) = tag_before(rest, at) {
                if let Some(stop) = body.find(&format!("</{tag}>")) {
                    found.insert(name.to_owned(), body[..stop].to_owned());
                }
            }
        }

        rest = &rest[after..];
    }

    found
}

fn tag_before(text: &str, at: usize) -> Option<&str> {
    let open = text[..at].rfind('<')?;
    let tag = text[open + 1..at].trim();
    if tag.is_empty() || tag.contains('<') {
        None
    } else {
        Some(tag)
    }
}

pub fn patch(text: &str, changes: &[Change]) -> String {
    let mut out = text.to_owned();
    for change in changes {
        match change.kind {
            Kind::Vector2 => set_point(&mut out, change),
            _ => set_scalar(&mut out, change),
        }
    }
    out
}

fn set_scalar(text: &mut String, change: &Change) {
    let needle = format!("name=\"{}\">", change.name);
    let Some(at) = text.find(&needle) else {
        insert(text, change);
        return;
    };
    let Some(tag) = tag_before(text, at).map(str::to_owned) else {
        return;
    };
    let start = at + needle.len();
    let Some(stop) = text[start..].find(&format!("</{tag}>")) else {
        return;
    };
    text.replace_range(start..start + stop, &change.value);
}

fn set_point(text: &mut String, change: &Change) {
    let needle = format!("name=\"{}\">", change.name);
    let Some(at) = text.find(&needle) else {
        return;
    };
    let start = at + needle.len();
    let Some(stop) = text[start..].find("</Vector2>") else {
        return;
    };

    let mut block = text[start..start + stop].to_owned();
    for axis in ["X", "Y"] {
        let open = format!("<{axis}>");
        let close = format!("</{axis}>");
        let Some(from) = block.find(&open) else {
            continue;
        };
        let value_at = from + open.len();
        let Some(to) = block[value_at..].find(&close) else {
            continue;
        };
        block.replace_range(value_at..value_at + to, &change.value);
    }

    text.replace_range(start..start + stop, &block);
}

fn insert(text: &mut String, change: &Change) {
    let Some(at) = text.find(PROPERTIES_END) else {
        return;
    };
    let line_start = text[..at].rfind('\n').map(|index| index + 1).unwrap_or(at);
    let indent = text[line_start..at].to_owned();
    let tag = change.kind.tag();
    let line = format!(
        "{indent}\t<{tag} name=\"{}\">{}</{tag}>\n",
        change.name, change.value
    );
    text.insert_str(line_start, &line);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Missing,
    Unchanged,
    Written,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub path: PathBuf,
    pub outcome: Outcome,
    pub backup: Option<PathBuf>,
    pub locked: bool,
    pub count: usize,
}

impl Report {
    pub fn summary(&self) -> String {
        match self.outcome {
            Outcome::Missing => "Roblox has not written its settings file yet".to_owned(),
            Outcome::Unchanged => format!("{} game settings already match", self.count),
            Outcome::Written => format!("wrote {} game settings", self.count),
        }
    }
}

pub fn apply(path: &Path, changes: &[Change], lock: bool, backup_dir: &Path) -> Result<Report> {
    let mut report = Report {
        path: path.to_path_buf(),
        outcome: Outcome::Missing,
        backup: None,
        locked: false,
        count: changes.len(),
    };

    let Some(text) = fs::read_to_string_if_exists(path)? else {
        return Ok(report);
    };

    if fs::is_read_only(path) {
        fs::set_read_only(path, false)?;
    }

    let patched = patch(&text, changes);
    if patched == text {
        report.outcome = Outcome::Unchanged;
    } else {
        report.backup = fs::back_up(path, backup_dir, BACKUPS_KEPT)?;
        fs::write_atomic(path, patched.as_bytes())?;
        report.outcome = Outcome::Written;
    }

    if lock {
        fs::set_read_only(path, true)?;
        report.locked = true;
    }

    Ok(report)
}

pub fn unlock(path: &Path) -> Result<bool> {
    if !path.is_file() || !fs::is_read_only(path) {
        return Ok(false);
    }
    fs::set_read_only(path, false)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = concat!(
        "<roblox version=\"4\">\n",
        "\t<Item class=\"UserGameSettings\" referent=\"RBX0\">\n",
        "\t\t<Properties>\n",
        "\t\t\t<int name=\"FramerateCap\">60</int>\n",
        "\t\t\t<bool name=\"Fullscreen\">true</bool>\n",
        "\t\t\t<float name=\"MouseSensitivity\">0.5</float>\n",
        "\t\t\t<Vector2 name=\"MouseSensitivityFirstPerson\">\n",
        "\t\t\t\t<X>0.5</X>\n",
        "\t\t\t\t<Y>0.5</Y>\n",
        "\t\t\t</Vector2>\n",
        "\t\t\t<token name=\"SavedQualityLevel\">0</token>\n",
        "\t\t</Properties>\n",
        "\t</Item>\n",
        "</roblox>\n",
    );

    #[test]
    fn reads_every_property_it_finds() {
        let values = scan(SAMPLE);
        assert_eq!(values.get("FramerateCap").unwrap(), "60");
        assert_eq!(values.get("Fullscreen").unwrap(), "true");
        assert_eq!(values.get("SavedQualityLevel").unwrap(), "0");
    }

    #[test]
    fn typed_reads_come_back_in_the_right_shape() {
        let snapshot = Snapshot {
            values: scan(SAMPLE),
            found: true,
            ..Snapshot::default()
        };

        assert_eq!(snapshot.int(FRAMERATE_CAP), Some(60));
        assert_eq!(snapshot.flag("Fullscreen"), Some(true));
        assert_eq!(snapshot.float(MOUSE_SENSITIVITY), Some(0.5));
        assert_eq!(snapshot.int("NotThere"), None);
    }

    #[test]
    fn changing_a_value_leaves_the_rest_of_the_file_alone() {
        let patched = patch(SAMPLE, &[Change::int(FRAMERATE_CAP, 240)]);

        assert!(patched.contains("<int name=\"FramerateCap\">240</int>"));
        assert!(patched.contains("<bool name=\"Fullscreen\">true</bool>"));
        assert_eq!(patched.lines().count(), SAMPLE.lines().count());
    }

    #[test]
    fn a_property_that_is_missing_is_added_to_the_block() {
        let patched = patch(SAMPLE, &[Change::flag(PERFORMANCE_STATS, true)]);

        assert!(patched.contains("\t\t\t<bool name=\"PerformanceStatsVisible\">true</bool>\n"));
        assert!(patched.contains(PROPERTIES_END));
        assert!(patched.contains("<int name=\"FramerateCap\">60</int>"));
    }

    #[test]
    fn both_axes_of_a_vector_are_written() {
        let patched = patch(SAMPLE, &[Change::point(MOUSE_FIRST_PERSON, 0.25)]);

        assert!(patched.contains("<X>0.25</X>"));
        assert!(patched.contains("<Y>0.25</Y>"));
        assert!(patched.contains("<float name=\"MouseSensitivity\">0.5</float>"));
    }

    #[test]
    fn a_name_is_never_confused_with_a_longer_one() {
        let patched = patch(SAMPLE, &[Change::float(MOUSE_SENSITIVITY, 0.75)]);

        assert!(patched.contains("<float name=\"MouseSensitivity\">0.75</float>"));
        assert!(patched.contains("<X>0.5</X>"));
    }

    #[test]
    fn writing_the_same_values_twice_changes_nothing() {
        let once = patch(SAMPLE, &[Change::int(FRAMERATE_CAP, 240)]);
        let twice = patch(&once, &[Change::int(FRAMERATE_CAP, 240)]);
        assert_eq!(once, twice);
    }

    #[test]
    fn floats_always_read_back_as_floats() {
        assert_eq!(render_float(1.0), "1.0");
        assert_eq!(render_float(0.25), "0.25");
        assert_eq!(Change::float("X", 2.0).value, "2.0");
    }

    #[test]
    fn a_missing_file_is_reported_rather_than_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("GlobalBasicSettings_13.xml");

        let report = apply(&path, &[Change::int(FRAMERATE_CAP, 240)], false, dir.path()).unwrap();

        assert_eq!(report.outcome, Outcome::Missing);
        assert!(!path.exists());
    }

    #[test]
    fn applying_backs_the_original_up_once_and_then_stays_quiet() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("GlobalBasicSettings_13.xml");
        let backups = dir.path().join("backups");
        fs::write_atomic(&path, SAMPLE.as_bytes()).unwrap();

        let first = apply(&path, &[Change::int(FRAMERATE_CAP, 240)], false, &backups).unwrap();
        assert_eq!(first.outcome, Outcome::Written);
        assert!(first.backup.is_some());

        let again = apply(&path, &[Change::int(FRAMERATE_CAP, 240)], false, &backups).unwrap();
        assert_eq!(again.outcome, Outcome::Unchanged);
        assert!(again.backup.is_none());

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("<int name=\"FramerateCap\">240</int>"));
    }

    #[test]
    fn locking_survives_the_next_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("GlobalBasicSettings_13.xml");
        fs::write_atomic(&path, SAMPLE.as_bytes()).unwrap();

        let locked = apply(&path, &[Change::int(FRAMERATE_CAP, 120)], true, dir.path()).unwrap();
        assert!(locked.locked);
        assert!(fs::is_read_only(&path));

        let again = apply(&path, &[Change::int(FRAMERATE_CAP, 144)], true, dir.path()).unwrap();
        assert_eq!(again.outcome, Outcome::Written);
        assert!(fs::is_read_only(&path));
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("<int name=\"FramerateCap\">144</int>"));

        fs::set_read_only(&path, false).unwrap();
    }

    #[test]
    fn turning_the_lock_off_clears_the_attribute() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("GlobalBasicSettings_13.xml");
        fs::write_atomic(&path, SAMPLE.as_bytes()).unwrap();

        apply(&path, &[Change::int(FRAMERATE_CAP, 120)], true, dir.path()).unwrap();
        let freed = apply(&path, &[Change::int(FRAMERATE_CAP, 120)], false, dir.path()).unwrap();

        assert_eq!(freed.outcome, Outcome::Unchanged);
        assert!(!freed.locked);
        assert!(!fs::is_read_only(&path));
    }

    #[test]
    fn unlocking_reports_whether_it_had_to_do_anything() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("GlobalBasicSettings_13.xml");
        fs::write_atomic(&path, SAMPLE.as_bytes()).unwrap();

        assert!(!unlock(&path).unwrap());
        fs::set_read_only(&path, true).unwrap();
        assert!(unlock(&path).unwrap());
        assert!(!fs::is_read_only(&path));
    }

    #[test]
    fn the_newest_numbered_file_wins() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "GlobalBasicSettings_9.xml",
            "GlobalBasicSettings_13.xml",
            "GlobalBasicSettings_notanumber.xml",
        ] {
            fs::write_atomic(&dir.path().join(name), b"<roblox/>").unwrap();
        }

        let found = newest_in(dir.path()).unwrap();
        assert_eq!(found.file_name().unwrap(), "GlobalBasicSettings_13.xml");
    }

    #[test]
    fn a_snapshot_of_a_missing_file_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let snapshot = Snapshot::read_from(&dir.path().join("nothing.xml"));

        assert!(!snapshot.found);
        assert!(!snapshot.locked);
        assert!(snapshot.values.is_empty());
    }
}
