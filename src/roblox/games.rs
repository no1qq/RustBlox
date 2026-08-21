use std::io::Read;
use std::time::Duration;

use serde_json::Value;

use crate::error::{Error, Result};

const TIMEOUT: Duration = Duration::from_secs(10);
const LIMIT: usize = 64 * 1024;
const GAMES: &str = "https://games.roblox.com/v1/games";
const UNIVERSES: &str = "https://apis.roblox.com/universes/v1/places";

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("RustBlox/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn get_text(url: &str) -> Result<String> {
    let mut response = agent()
        .get(url)
        .call()
        .map_err(|err| Error::invalid(format!("{url} could not be reached: {err}")))?;

    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .take(LIMIT as u64)
        .read_to_string(&mut body)
        .map_err(|err| Error::io(format!("could not read the response from {url}"), err))?;

    Ok(body)
}

pub fn universe_of(place_id: u64) -> Result<u64> {
    let body = get_text(&format!("{UNIVERSES}/{place_id}/universe"))?;
    parse_universe(&body).ok_or_else(|| Error::invalid(format!("place {place_id} has no universe")))
}

pub fn name_of(universe_id: u64) -> Result<String> {
    let body = get_text(&format!("{GAMES}?universeIds={universe_id}"))?;
    parse_name(&body).ok_or_else(|| Error::invalid(format!("universe {universe_id} has no name")))
}

fn parse_universe(body: &str) -> Option<u64> {
    serde_json::from_str::<Value>(body)
        .ok()?
        .get("universeId")?
        .as_u64()
}

fn parse_name(body: &str) -> Option<String> {
    let value: Value = serde_json::from_str(body).ok()?;
    let name = value
        .get("data")?
        .as_array()?
        .first()?
        .get("name")?
        .as_str()?
        .trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_universe_is_read_out_of_the_answer() {
        assert_eq!(
            parse_universe(r#"{"universeId":5069824722}"#),
            Some(5069824722)
        );
        assert_eq!(parse_universe(r#"{"universeId":null}"#), None);
        assert_eq!(parse_universe("not json"), None);
    }

    #[test]
    fn a_name_is_read_out_of_the_answer() {
        let body = r#"{"data":[{"id":5069824722,"rootPlaceId":14705961406,"name":"Some Game"}]}"#;
        assert_eq!(parse_name(body), Some("Some Game".to_owned()));
    }

    #[test]
    fn an_empty_list_has_no_name() {
        assert_eq!(parse_name(r#"{"data":[]}"#), None);
        assert_eq!(parse_name(r#"{"data":[{"name":"   "}]}"#), None);
        assert_eq!(parse_name("{}"), None);
    }

    #[test]
    #[ignore = "reaches the Roblox web API"]
    fn a_real_place_resolves_to_a_real_name() {
        let universe = universe_of(1818).unwrap();
        let name = name_of(universe).unwrap();
        assert!(!name.is_empty(), "{universe} came back nameless");
    }
}
