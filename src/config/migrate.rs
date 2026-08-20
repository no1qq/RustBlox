use serde_json::{Map, Value};

pub const CURRENT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Migration {
    UpToDate,
    Upgraded { from: u32, steps: Vec<&'static str> },
    TooNew { found: u32 },
}

impl Migration {
    pub fn note(&self) -> Option<String> {
        match self {
            Migration::UpToDate => None,
            Migration::Upgraded { from, steps } if steps.is_empty() => {
                Some(format!("settings stamped with format version {CURRENT_VERSION} (was {from})"))
            }
            Migration::Upgraded { from, steps } => Some(format!(
                "settings upgraded from format version {from}: {}",
                steps.join(", ")
            )),
            Migration::TooNew { found } => Some(format!(
                "settings were written by a newer RustBlox (format {found}, this build understands {CURRENT_VERSION}); defaults are in use so the existing file is left untouched"
            )),
        }
    }
}

type Step = fn(&mut Map<String, Value>);

fn steps_for(version: u32) -> Option<(&'static str, Step)> {
    match version {
        0 => Some(("adopted the versioned settings layout", step_v0_to_v1)),
        _ => None,
    }
}

fn step_v0_to_v1(root: &mut Map<String, Value>) {
    for section in ["launch", "appearance", "advanced"] {
        if !root.get(section).map(Value::is_object).unwrap_or(false) {
            root.insert(section.into(), Value::Object(Map::new()));
        }
    }
}

pub fn migrate(value: &mut Value) -> Migration {
    let Some(root) = value.as_object_mut() else {
        *value = Value::Object(Map::new());
        return Migration::Upgraded {
            from: 0,
            steps: vec!["replaced a non-object settings document"],
        };
    };

    let found = root
        .get("version")
        .and_then(Value::as_u64)
        .unwrap_or(0)
        .min(u32::MAX as u64) as u32;

    if found > CURRENT_VERSION {
        return Migration::TooNew { found };
    }
    if found == CURRENT_VERSION {
        return Migration::UpToDate;
    }

    let mut applied = Vec::new();
    let mut version = found;
    while version < CURRENT_VERSION {
        match steps_for(version) {
            Some((label, step)) => {
                step(root);
                applied.push(label);
            }
            None => break,
        }
        version += 1;
    }

    root.insert("version".into(), Value::from(CURRENT_VERSION));
    Migration::Upgraded {
        from: found,
        steps: applied,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_current_document_is_left_alone() {
        let mut value = json!({"version": CURRENT_VERSION});
        assert_eq!(migrate(&mut value), Migration::UpToDate);
        assert!(migrate(&mut value).note().is_none());
    }

    #[test]
    fn an_unversioned_document_gains_the_sections_and_a_stamp() {
        let mut value = json!({});
        let outcome = migrate(&mut value);

        assert!(matches!(outcome, Migration::Upgraded { from: 0, .. }));
        assert_eq!(value["version"], json!(CURRENT_VERSION));
        assert!(value["launch"].is_object());
        assert!(value["appearance"].is_object());
        assert!(value["advanced"].is_object());
        assert!(outcome.note().unwrap().contains("upgraded"));
    }

    #[test]
    fn existing_sections_are_preserved_while_migrating() {
        let mut value = json!({"launch": {"close_after_launch": true}});
        migrate(&mut value);
        assert_eq!(value["launch"]["close_after_launch"], json!(true));
    }

    #[test]
    fn a_future_version_is_refused_rather_than_downgraded() {
        let mut value = json!({"version": CURRENT_VERSION + 5, "launch": {}});
        let outcome = migrate(&mut value);

        assert_eq!(
            outcome,
            Migration::TooNew {
                found: CURRENT_VERSION + 5
            }
        );
        assert_eq!(value["version"], json!(CURRENT_VERSION + 5));
        assert!(outcome.note().unwrap().contains("newer RustBlox"));
    }

    #[test]
    fn a_non_object_document_is_replaced() {
        let mut value = json!([1, 2, 3]);
        let outcome = migrate(&mut value);
        assert!(matches!(outcome, Migration::Upgraded { from: 0, .. }));
        assert!(value.is_object());
    }
}
