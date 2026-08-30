use std::io::Read;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::util::fs;

const TIMEOUT: Duration = Duration::from_secs(10);
const LIMIT: usize = 256 * 1024;

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("RustBlox/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountProfile {
    pub id: u64,
    pub username: String,
    pub display_name: String,
    pub cookie: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct QuickSignInSession {
    pub code: String,
    pub private_key: String,
    pub created_at: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickSignInPollResult {
    Pending,
    Approved(String),
    Denied,
    Expired,
    Error(String),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct FriendInfo {
    pub id: u64,
    pub username: String,
    pub display_name: String,
    pub is_online: bool,
    pub presence_type: u8,
    pub last_location: Option<String>,
    pub place_id: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserGameInfo {
    pub id: u64,
    pub name: String,
    pub place_id: u64,
    pub creator_name: String,
}

pub fn sanitize_cookie(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim();
    let cleaned = trimmed
        .strip_prefix(".ROBLOSECURITY=")
        .unwrap_or(trimmed)
        .trim_matches('"')
        .trim();
    cleaned.to_string()
}

pub fn fetch_account_details(cookie: &str) -> Result<(u64, String, String)> {
    let clean = sanitize_cookie(cookie);
    if clean.is_empty() {
        return Err(Error::invalid("Please enter a valid .ROBLOSECURITY cookie"));
    }

    let cookie_header = format!(".ROBLOSECURITY={clean}");
    let mut response = agent()
        .get("https://users.roblox.com/v1/users/authenticated")
        .header("Cookie", &cookie_header)
        .call()
        .map_err(|err| Error::invalid(format!("Invalid or expired Roblox session: {err}")))?;

    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .take(LIMIT as u64)
        .read_to_string(&mut body)
        .map_err(|err| Error::io("Could not read user details", err))?;

    let value: Value = serde_json::from_str(&body)
        .map_err(|err| Error::invalid(format!("Could not parse user details: {err}")))?;

    let id = value
        .get("id")
        .and_then(Value::as_u64)
        .ok_or_else(|| Error::invalid("Missing user ID in response"))?;
    let username = value
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Roblox Player")
        .to_string();
    let display_name = value
        .get("displayName")
        .and_then(Value::as_str)
        .unwrap_or(&username)
        .to_string();

    Ok((id, username, display_name))
}

pub fn start_quick_sign_in() -> Result<QuickSignInSession> {
    let payload = serde_json::json!({
        "deviceType": "Computer",
        "deviceMetadata": "RustBlox"
    });
    let payload_str = payload.to_string();

    let mut response = agent()
        .post("https://auth.roblox.com/v1/cross-device/start")
        .header("Content-Type", "application/json")
        .send(payload_str.as_bytes())
        .map_err(|err| Error::invalid(format!("Could not initiate Quick Sign-In: {err}")))?;

    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .take(LIMIT as u64)
        .read_to_string(&mut body)
        .map_err(|err| Error::io("Could not read Quick Sign-In response", err))?;

    let value: Value = serde_json::from_str(&body)
        .map_err(|err| Error::invalid(format!("Invalid response format: {err}")))?;

    let code = value
        .get("code")
        .and_then(Value::as_str)
        .ok_or_else(|| Error::invalid("Quick Sign-In code was not returned"))?
        .to_string();
    let private_key = value
        .get("privateKey")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();

    Ok(QuickSignInSession {
        code,
        private_key,
        created_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
    })
}

pub fn poll_quick_sign_in(session: &QuickSignInSession) -> Result<QuickSignInPollResult> {
    let payload = serde_json::json!({
        "privateKey": session.private_key
    });
    let payload_str = payload.to_string();

    let mut response = match agent()
        .post("https://auth.roblox.com/v1/cross-device/poll")
        .header("Content-Type", "application/json")
        .send(payload_str.as_bytes())
    {
        Ok(res) => res,
        Err(err) => return Ok(QuickSignInPollResult::Error(err.to_string())),
    };

    let mut body = String::new();
    let _ = response
        .body_mut()
        .as_reader()
        .take(LIMIT as u64)
        .read_to_string(&mut body);

    let Ok(value) = serde_json::from_str::<Value>(&body) else {
        return Ok(QuickSignInPollResult::Pending);
    };

    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("Pending");
    match status {
        "Approved" => {
            if let Some(cookie) = value.get("cookie").and_then(Value::as_str) {
                Ok(QuickSignInPollResult::Approved(sanitize_cookie(cookie)))
            } else if let Some(cookie) = value.get("token").and_then(Value::as_str) {
                Ok(QuickSignInPollResult::Approved(sanitize_cookie(cookie)))
            } else {
                Ok(QuickSignInPollResult::Approved(String::new()))
            }
        }
        "Denied" => Ok(QuickSignInPollResult::Denied),
        "Expired" => Ok(QuickSignInPollResult::Expired),
        _ => Ok(QuickSignInPollResult::Pending),
    }
}

pub fn fetch_friends(user_id: u64, cookie: &str) -> Result<Vec<FriendInfo>> {
    let clean = sanitize_cookie(cookie);
    let cookie_header = format!(".ROBLOSECURITY={clean}");

    let mut request = agent().get(format!(
        "https://friends.roblox.com/v1/users/{user_id}/friends"
    ));
    if !clean.is_empty() {
        request = request.header("Cookie", &cookie_header);
    }

    let mut response = request
        .call()
        .map_err(|err| Error::invalid(format!("Could not fetch friends list: {err}")))?;

    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .take(LIMIT as u64)
        .read_to_string(&mut body)
        .map_err(|err| Error::io("Could not read friends data", err))?;

    let value: Value = serde_json::from_str(&body)
        .map_err(|err| Error::invalid(format!("Invalid friends payload: {err}")))?;

    let mut friends = Vec::new();
    let Some(data) = value.get("data").and_then(Value::as_array) else {
        return Ok(friends);
    };

    let mut ids = Vec::new();
    for item in data {
        let Some(id) = item.get("id").and_then(Value::as_u64) else {
            continue;
        };
        let username = item
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("Friend")
            .to_string();
        let display_name = item
            .get("displayName")
            .and_then(Value::as_str)
            .unwrap_or(&username)
            .to_string();
        let is_online = item
            .get("isOnline")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        ids.push(id);
        friends.push(FriendInfo {
            id,
            username,
            display_name,
            is_online,
            presence_type: if is_online { 1 } else { 0 },
            last_location: None,
            place_id: None,
        });
    }

    if !ids.is_empty() {
        let presence_payload = serde_json::json!({
            "userIds": ids.iter().take(100).copied().collect::<Vec<_>>()
        });
        let mut pres_req = agent().post("https://presence.roblox.com/v1/presence/users");
        if !clean.is_empty() {
            pres_req = pres_req.header("Cookie", &cookie_header);
        }
        if let Ok(mut pres_res) = pres_req
            .header("Content-Type", "application/json")
            .send(presence_payload.to_string().as_bytes())
        {
            let mut pres_body = String::new();
            if pres_res
                .body_mut()
                .as_reader()
                .take(LIMIT as u64)
                .read_to_string(&mut pres_body)
                .is_ok()
            {
                if let Ok(pres_val) = serde_json::from_str::<Value>(&pres_body) {
                    if let Some(pres_array) =
                        pres_val.get("userPresences").and_then(Value::as_array)
                    {
                        for pres in pres_array {
                            let Some(uid) = pres.get("userId").and_then(Value::as_u64) else {
                                continue;
                            };
                            let p_type = pres
                                .get("userPresenceType")
                                .and_then(Value::as_u64)
                                .unwrap_or(0) as u8;
                            let last_loc = pres
                                .get("lastLocation")
                                .and_then(Value::as_str)
                                .map(|s| s.to_string());
                            let place_id = pres.get("placeId").and_then(Value::as_u64);

                            if let Some(friend) = friends.iter_mut().find(|f| f.id == uid) {
                                friend.presence_type = p_type;
                                friend.is_online = p_type > 0;
                                friend.last_location = last_loc;
                                friend.place_id = place_id;
                            }
                        }
                    }
                }
            }
        }
    }

    friends.sort_by_key(|b| std::cmp::Reverse(b.presence_type));
    Ok(friends)
}

pub fn fetch_user_games(user_id: u64, cookie: &str) -> Result<Vec<UserGameInfo>> {
    let clean = sanitize_cookie(cookie);
    let cookie_header = format!(".ROBLOSECURITY={clean}");

    let mut request = agent().get(format!(
        "https://games.roblox.com/v2/users/{user_id}/games?sortOrder=Desc&limit=50"
    ));
    if !clean.is_empty() {
        request = request.header("Cookie", &cookie_header);
    }

    let mut response = request
        .call()
        .map_err(|err| Error::invalid(format!("Could not fetch games: {err}")))?;

    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .take(LIMIT as u64)
        .read_to_string(&mut body)
        .map_err(|err| Error::io("Could not read games data", err))?;

    let value: Value = serde_json::from_str(&body)
        .map_err(|err| Error::invalid(format!("Invalid games response: {err}")))?;

    let mut games = Vec::new();
    if let Some(data) = value.get("data").and_then(Value::as_array) {
        for item in data {
            let Some(id) = item.get("id").and_then(Value::as_u64) else {
                continue;
            };
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("Untitled Game")
                .to_string();
            let place_id = item
                .get("rootPlaceId")
                .and_then(Value::as_u64)
                .or_else(|| {
                    item.get("rootPlace")
                        .and_then(|rp| rp.get("id"))
                        .and_then(Value::as_u64)
                })
                .unwrap_or(id);
            let creator_name = item
                .get("creator")
                .and_then(|c| c.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("Creator")
                .to_string();

            games.push(UserGameInfo {
                id,
                name,
                place_id,
                creator_name,
            });
        }
    }

    Ok(games)
}

#[cfg(windows)]
pub fn apply_account_session(cookie: &str) -> Result<()> {
    let clean = sanitize_cookie(cookie);
    if clean.is_empty() {
        return Ok(());
    }
    let formatted = format!(".ROBLOSECURITY={clean}");

    let hkcu = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
    if let Ok((key, _)) =
        hkcu.create_subkey(r"Software\ROBLOX Corporation\Environments\roblox-player")
    {
        let _ = key.set_value("robloxcookies", &formatted);
    }

    if let Some(local_dir) = crate::roblox::local_dir() {
        let cookies_dir = local_dir.join("LocalStorage");
        if cookies_dir.is_dir() {
            let cookies_file = cookies_dir.join("RobloxCookies.dat");
            let _ = fs::write_atomic(&cookies_file, formatted.as_bytes());
        }
    }

    Ok(())
}

#[cfg(not(windows))]
pub fn apply_account_session(_cookie: &str) -> Result<()> {
    Ok(())
}

pub fn load_accounts(path: &Path) -> Result<Vec<AccountProfile>> {
    match fs::read_to_string_if_exists(path)? {
        Some(text) => serde_json::from_str::<Vec<AccountProfile>>(&text)
            .map_err(|err| Error::invalid(format!("Accounts file is invalid: {err}"))),
        None => Ok(Vec::new()),
    }
}

pub fn save_accounts(path: &Path, accounts: &[AccountProfile]) -> Result<()> {
    let mut text = serde_json::to_string_pretty(accounts)
        .map_err(|err| Error::invalid(format!("Could not serialize accounts: {err}")))?;
    text.push('\n');
    fs::write_atomic(path, text.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_roblox_cookies() {
        assert_eq!(
            sanitize_cookie("  .ROBLOSECURITY=_|WARNING:-DO-NOT-SHARE-THIS...  "),
            "_|WARNING:-DO-NOT-SHARE-THIS..."
        );
        assert_eq!(sanitize_cookie(r#"".ROBLOSECURITY=abc""#), "abc");
    }

    #[test]
    fn accounts_round_trip_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("accounts.json");

        let accounts = vec![AccountProfile {
            id: 12345,
            username: "Player1".into(),
            display_name: "ProPlayer".into(),
            cookie: "token_123".into(),
            created_at: "2026-08-30".into(),
        }];

        save_accounts(&file, &accounts).unwrap();
        let loaded = load_accounts(&file).unwrap();
        assert_eq!(loaded, accounts);
    }
}
