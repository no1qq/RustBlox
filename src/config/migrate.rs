use serde_json::{Map, Value};

pub const CURRENT_VERSION: u32 = 6;

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
        1 => Some((
            "replaced the named themes with a light and dark mode",
            step_v1_to_v2,
        )),
        2 => Some((
            "turned on writing the flag profile on every launch",
            step_v2_to_v3,
        )),
        3 => Some((
            "added the Roblox game settings section, left switched off",
            step_v3_to_v4,
        )),
        4 => Some(("added the mods section, left switched off", step_v4_to_v5)),
        5 => Some((
            "added the Discord section, left switched off",
            step_v5_to_v6,
        )),
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

fn step_v1_to_v2(root: &mut Map<String, Value>) {
    let Some(appearance) = root.get_mut("appearance").and_then(Value::as_object_mut) else {
        return;
    };

    let mode = match appearance.remove("theme").as_ref().and_then(Value::as_str) {
        Some("Daylight") => "Light",
        Some("Midnight") | Some("Graphite") => "Dark",
        _ => "Auto",
    };

    appearance
        .entry("mode")
        .or_insert_with(|| Value::from(mode));
}

fn step_v2_to_v3(root: &mut Map<String, Value>) {
    let advanced = root
        .entry("advanced")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(advanced) = advanced.as_object_mut() {
        advanced.insert("apply_flag_profile".into(), Value::Bool(true));
    }
}

fn step_v3_to_v4(root: &mut Map<String, Value>) {
    let game = root
        .entry("game")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(game) = game.as_object_mut() {
        game.entry("manage").or_insert(Value::Bool(false));
        game.entry("lock").or_insert(Value::Bool(false));
    }
}

fn step_v4_to_v5(root: &mut Map<String, Value>) {
    let mods = root
        .entry("mods")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(mods) = mods.as_object_mut() {
        mods.entry("enabled").or_insert(Value::Bool(false));
    }
}

fn step_v5_to_v6(root: &mut Map<String, Value>) {
    let discord = root
        .entry("discord")
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(discord) = discord.as_object_mut() {
        discord.entry("enabled").or_insert(Value::Bool(false));
        discord
            .entry("show_place_name")
            .or_insert(Value::Bool(true));
        discord
            .entry("application_id")
            .or_insert(Value::from(crate::discord::DEFAULT_APPLICATION_ID));
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
    fn the_light_theme_becomes_light_mode() {
        let mut value =
            json!({"version": 1, "appearance": {"theme": "Daylight", "accent": "Lime"}});
        migrate(&mut value);

        assert_eq!(value["appearance"]["mode"], json!("Light"));
        assert_eq!(value["appearance"]["accent"], json!("Lime"));
        assert!(value["appearance"].get("theme").is_none());
    }

    #[test]
    fn the_dark_themes_become_dark_mode() {
        for named in ["Midnight", "Graphite"] {
            let mut value = json!({"version": 1, "appearance": {"theme": named}});
            migrate(&mut value);
            assert_eq!(value["appearance"]["mode"], json!("Dark"), "{named}");
        }
    }

    #[test]
    fn an_unknown_theme_falls_back_to_following_windows() {
        let mut value = json!({"version": 1, "appearance": {"theme": "Neon"}});
        migrate(&mut value);
        assert_eq!(value["appearance"]["mode"], json!("Auto"));
    }

    #[test]
    fn a_file_with_no_theme_at_all_still_migrates() {
        let mut value = json!({"version": 1, "appearance": {}});
        migrate(&mut value);
        assert_eq!(value["appearance"]["mode"], json!("Auto"));
        assert_eq!(value["version"], json!(CURRENT_VERSION));
    }

    #[test]
    fn an_unversioned_file_walks_every_step() {
        let mut value = json!({"appearance": {"theme": "Graphite"}});
        let outcome = migrate(&mut value);

        assert!(matches!(outcome, Migration::Upgraded { from: 0, .. }));
        assert_eq!(value["appearance"]["mode"], json!("Dark"));
        assert_eq!(value["version"], json!(CURRENT_VERSION));
    }

    #[test]
    fn writing_flags_on_launch_is_turned_on_when_upgrading() {
        let mut value = json!({"version": 2, "advanced": {"apply_flag_profile": false}});
        let outcome = migrate(&mut value);

        assert_eq!(value["advanced"]["apply_flag_profile"], json!(true));
        assert!(outcome.note().unwrap().contains("flag profile"));
    }

    #[test]
    fn the_game_settings_section_arrives_switched_off() {
        let mut value = json!({"version": 3, "advanced": {"channel": "zflag"}});
        let outcome = migrate(&mut value);

        assert_eq!(value["game"]["manage"], json!(false));
        assert_eq!(value["game"]["lock"], json!(false));
        assert_eq!(value["advanced"]["channel"], json!("zflag"));
        assert!(outcome.note().unwrap().contains("game settings"));
    }

    #[test]
    fn the_mods_section_arrives_switched_off() {
        let mut value = json!({"version": 4});
        let outcome = migrate(&mut value);

        assert_eq!(value["mods"]["enabled"], json!(false));
        assert!(outcome.note().unwrap().contains("mods"));
    }

    #[test]
    fn the_discord_section_arrives_switched_off() {
        let mut value = json!({"version": 5});
        let outcome = migrate(&mut value);

        assert_eq!(value["discord"]["enabled"], json!(false));
        assert_eq!(value["discord"]["show_place_name"], json!(true));
        assert_eq!(
            value["discord"]["application_id"],
            json!(crate::discord::DEFAULT_APPLICATION_ID)
        );
        assert!(outcome.note().unwrap().contains("Discord"));
    }

    #[test]
    fn an_application_id_that_is_already_there_is_kept() {
        let mut value = json!({"version": 5, "discord": {"application_id": "999999999999999999"}});
        migrate(&mut value);

        assert_eq!(
            value["discord"]["application_id"],
            json!("999999999999999999")
        );
    }

    #[test]
    fn an_existing_game_section_is_left_as_it_was() {
        let mut value = json!({"version": 3, "game": {"manage": true, "framerate_cap": 240}});
        migrate(&mut value);

        assert_eq!(value["game"]["manage"], json!(true));
        assert_eq!(value["game"]["framerate_cap"], json!(240));
    }

    #[test]
    fn upgrading_from_two_keeps_the_rest_of_the_advanced_section() {
        let mut value = json!({"version": 2, "advanced": {"channel": "zflag"}});
        migrate(&mut value);
        assert_eq!(value["advanced"]["channel"], json!("zflag"));
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
