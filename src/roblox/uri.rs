use crate::error::{Error, Result};

pub const SCHEME_PLAYER: &str = "roblox-player";
pub const SCHEME_DEEPLINK: &str = "roblox";
const MAX_URI_LEN: usize = 4096;

pub fn deep_link(place_id: u64) -> String {
    format!("roblox://experiences/start?placeId={place_id}")
}

pub fn is_launch_uri(value: &str) -> bool {
    let lowered = value.trim().to_ascii_lowercase();
    lowered.starts_with("roblox:") || lowered.starts_with("roblox-player:")
}

pub fn validate(value: &str) -> Result<String> {
    let trimmed = value.trim();

    if trimmed.is_empty() {
        return Err(Error::invalid("the launch link is empty"));
    }
    if trimmed.len() > MAX_URI_LEN {
        return Err(Error::invalid(format!(
            "the launch link is {} characters long, which exceeds the {MAX_URI_LEN} character limit",
            trimmed.len()
        )));
    }
    if !is_launch_uri(trimmed) {
        return Err(Error::invalid(
            "only roblox: and roblox-player: links can be forwarded",
        ));
    }
    if trimmed.chars().any(|c| c.is_control()) {
        return Err(Error::invalid(
            "the launch link contains control characters and was rejected",
        ));
    }
    if trimmed.contains('"') {
        return Err(Error::invalid(
            "the launch link contains a quote character and was rejected",
        ));
    }

    Ok(trimmed.to_owned())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UriSummary {
    pub scheme: String,
    pub launch_mode: Option<String>,
    pub place_id: Option<u64>,
    pub has_ticket: bool,
}

impl UriSummary {
    pub fn headline(&self) -> String {
        if let Some(place) = self.place_id {
            return format!("Joining place {place}");
        }
        match self.launch_mode.as_deref() {
            Some("app") => "Opening the Roblox home screen".into(),
            Some("play") => "Joining an experience".into(),
            Some(other) => format!("Launch mode: {other}"),
            None => "Opening Roblox".into(),
        }
    }
}

pub fn summarise(uri: &str) -> UriSummary {
    let trimmed = uri.trim();
    let (scheme, rest) = match trimmed.split_once(':') {
        Some((scheme, rest)) => (scheme.to_ascii_lowercase(), rest),
        None => (String::new(), trimmed),
    };

    let mut summary = UriSummary {
        scheme,
        ..Default::default()
    };

    if summary.scheme == SCHEME_PLAYER {
        for pair in rest.split('+') {
            let Some((key, value)) = pair.split_once(':') else {
                continue;
            };
            match key.to_ascii_lowercase().as_str() {
                "launchmode" => summary.launch_mode = Some(value.to_ascii_lowercase()),
                "gameinfo" => summary.has_ticket = !value.is_empty(),
                "placelauncherurl" => {
                    summary.place_id = place_from_query(&percent_decode(value));
                }
                _ => {}
            }
        }
        if summary.place_id.is_none() {
            summary.place_id = place_from_query(&percent_decode(rest));
        }
    } else {
        summary.place_id = place_from_query(rest);
        if rest.contains("experiences/start") {
            summary.launch_mode = Some("play".into());
        }
    }

    summary
}

fn place_from_query(value: &str) -> Option<u64> {
    let lowered = value.to_ascii_lowercase();
    let index = lowered.find("placeid=")?;
    let tail = &value[index + "placeid=".len()..];
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).ok();
            if let Some(byte) = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn parse_place_input(input: &str) -> Result<u64> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(Error::invalid("enter a place ID or a Roblox game link"));
    }

    if let Ok(id) = trimmed.parse::<u64>() {
        if id == 0 {
            return Err(Error::invalid("0 is not a valid place ID"));
        }
        return Ok(id);
    }

    if let Some(id) = place_from_query(trimmed) {
        if id != 0 {
            return Ok(id);
        }
    }

    if let Some(id) = place_from_games_path(trimmed) {
        return Ok(id);
    }

    Err(Error::invalid(
        "that is not a place ID or a recognised roblox.com/games link",
    ))
}

fn place_from_games_path(value: &str) -> Option<u64> {
    let lowered = value.to_ascii_lowercase();
    let index = lowered.find("/games/")?;
    let tail = &value[index + "/games/".len()..];
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    let id = digits.parse::<u64>().ok()?;
    if id == 0 {
        None
    } else {
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_place_deep_link() {
        assert_eq!(deep_link(1818), "roblox://experiences/start?placeId=1818");
    }

    #[test]
    fn recognises_both_launch_schemes() {
        assert!(is_launch_uri("roblox://experiences/start?placeId=1"));
        assert!(is_launch_uri("ROBLOX-PLAYER:1+launchmode:app"));
        assert!(!is_launch_uri("https://roblox.com"));
    }

    #[test]
    fn validation_rejects_foreign_and_hostile_input() {
        assert!(validate("").is_err());
        assert!(validate("https://example.com").is_err());
        assert!(validate("roblox://a\nb").is_err());
        assert!(validate("roblox://a\"b").is_err());
        assert!(validate(&format!("roblox://{}", "a".repeat(MAX_URI_LEN))).is_err());
        assert_eq!(validate("  roblox://x  ").unwrap(), "roblox://x");
    }

    #[test]
    fn summarises_a_player_protocol_uri() {
        let summary = summarise("roblox-player:1+launchmode:play+gameinfo:TICKET+placelauncherurl:https%3A%2F%2Fx%2F%3FplaceId%3D42");
        assert_eq!(summary.scheme, "roblox-player");
        assert_eq!(summary.launch_mode.as_deref(), Some("play"));
        assert!(summary.has_ticket);
        assert_eq!(summary.place_id, Some(42));
        assert_eq!(summary.headline(), "Joining place 42");
    }

    #[test]
    fn summarises_an_app_launch() {
        let summary = summarise("roblox-player:1+launchmode:app");
        assert_eq!(summary.launch_mode.as_deref(), Some("app"));
        assert!(!summary.has_ticket);
        assert_eq!(summary.headline(), "Opening the Roblox home screen");
    }

    #[test]
    fn summarises_a_deep_link() {
        let summary = summarise("roblox://experiences/start?placeId=920587237");
        assert_eq!(summary.place_id, Some(920587237));
        assert_eq!(summary.launch_mode.as_deref(), Some("play"));
    }

    #[test]
    fn accepts_place_ids_and_game_links() {
        assert_eq!(parse_place_input("1818").unwrap(), 1818);
        assert_eq!(parse_place_input("  606849621 ").unwrap(), 606849621);
        assert_eq!(
            parse_place_input("https://www.roblox.com/games/1818/Classic-Chaos").unwrap(),
            1818
        );
        assert_eq!(
            parse_place_input("roblox://experiences/start?placeId=77").unwrap(),
            77
        );
        assert!(parse_place_input("0").is_err());
        assert!(parse_place_input("").is_err());
        assert!(parse_place_input("not a place").is_err());
    }
}
