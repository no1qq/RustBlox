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

fn raw_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("RustBlox/", env!("CARGO_PKG_VERSION")))
        .http_status_as_error(false)
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
    Pending(String),
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

pub fn sanitize_cookie(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches('"').trim();
    let cleaned = trimmed
        .strip_prefix(".ROBLOSECURITY=")
        .unwrap_or(trimmed)
        .trim_matches('"')
        .trim();
    if let Some(first_part) = cleaned.split(';').next() {
        first_part.trim().to_string()
    } else {
        cleaned.to_string()
    }
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
    let mut response = raw_agent()
        .post("https://apis.roblox.com/auth-token-service/v1/login/create")
        .header("Content-Type", "application/json")
        .send(b"{}")
        .map_err(|err| Error::invalid(format!("Could not initiate Quick Sign-In: {err}")))?;

    let mut body = String::new();
    let _ = response
        .body_mut()
        .as_reader()
        .take(LIMIT as u64)
        .read_to_string(&mut body);

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
        "code": session.code,
        "privateKey": session.private_key
    });
    let payload_str = payload.to_string();

    let mut res = raw_agent()
        .post("https://apis.roblox.com/auth-token-service/v1/login/status")
        .header("Content-Type", "application/json")
        .header("Origin", "https://www.roblox.com")
        .header("Referer", "https://www.roblox.com/")
        .send(payload_str.as_bytes())
        .map_err(|err| Error::invalid(format!("Could not check status: {err}")))?;

    if res.status().as_u16() == 403 {
        if let Some(csrf) = res
            .headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
        {
            res = raw_agent()
                .post("https://apis.roblox.com/auth-token-service/v1/login/status")
                .header("Content-Type", "application/json")
                .header("Origin", "https://www.roblox.com")
                .header("Referer", "https://www.roblox.com/")
                .header("x-csrf-token", csrf)
                .send(payload_str.as_bytes())
                .map_err(|err| Error::invalid(format!("Could not check status: {err}")))?;
        }
    }

    let mut body = String::new();
    let _ = res
        .body_mut()
        .as_reader()
        .take(LIMIT as u64)
        .read_to_string(&mut body);

    let Ok(value) = serde_json::from_str::<Value>(&body) else {
        return Ok(QuickSignInPollResult::Pending(
            "Waiting for approval on Roblox (enter code and click Grant Access)...".into(),
        ));
    };

    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or("Created");
    match status {
        "Created" => Ok(QuickSignInPollResult::Pending(
            "Ready - Enter code on Roblox and click Grant Access".into(),
        )),
        "UserLinked" => Ok(QuickSignInPollResult::Pending(
            "Linked! Click 'Grant Full Account Access' on Roblox...".into(),
        )),
        "Cancelled" | "Invalid" => Ok(QuickSignInPollResult::Denied),
        "Expired" => Ok(QuickSignInPollResult::Expired),
        "Validated" => {
            let login_payload = serde_json::json!({
                "ctype": "AuthToken",
                "cvalue": session.code,
                "password": session.private_key
            });
            let login_str = login_payload.to_string();

            let mut login_res = raw_agent()
                .post("https://auth.roblox.com/v2/login")
                .header("Content-Type", "application/json")
                .header("Origin", "https://www.roblox.com")
                .header("Referer", "https://www.roblox.com/")
                .send(login_str.as_bytes())
                .map_err(|err| Error::invalid(format!("Could not complete login: {err}")))?;

            if login_res.status().as_u16() == 403 {
                if let Some(csrf) = login_res
                    .headers()
                    .get("x-csrf-token")
                    .and_then(|v| v.to_str().ok())
                {
                    login_res = raw_agent()
                        .post("https://auth.roblox.com/v2/login")
                        .header("Content-Type", "application/json")
                        .header("Origin", "https://www.roblox.com")
                        .header("Referer", "https://www.roblox.com/")
                        .header("x-csrf-token", csrf)
                        .send(login_str.as_bytes())
                        .map_err(|err| {
                            Error::invalid(format!("Could not complete login: {err}"))
                        })?;
                }
            }

            for val in login_res.headers().get_all("set-cookie") {
                if let Ok(cookie_str) = val.to_str() {
                    if cookie_str.contains(".ROBLOSECURITY") {
                        return Ok(QuickSignInPollResult::Approved(sanitize_cookie(cookie_str)));
                    }
                }
            }

            for (name, val) in login_res.headers() {
                if name.as_str().eq_ignore_ascii_case("set-cookie") {
                    if let Ok(cookie_str) = val.to_str() {
                        if cookie_str.contains(".ROBLOSECURITY") {
                            return Ok(QuickSignInPollResult::Approved(sanitize_cookie(
                                cookie_str,
                            )));
                        }
                    }
                }
            }

            let mut login_body = String::new();
            let _ = login_res
                .body_mut()
                .as_reader()
                .take(LIMIT as u64)
                .read_to_string(&mut login_body);

            if let Ok(login_val) = serde_json::from_str::<Value>(&login_body) {
                if let Some(tok) = login_val
                    .get("token")
                    .or_else(|| login_val.get("cookie"))
                    .and_then(Value::as_str)
                {
                    return Ok(QuickSignInPollResult::Approved(sanitize_cookie(tok)));
                }
            }

            Ok(QuickSignInPollResult::Error(format!(
                "Validated, but login returned status {}: {login_body}",
                login_res.status().as_u16()
            )))
        }
        other => Ok(QuickSignInPollResult::Pending(format!("Status: {other}"))),
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

    let mut ids = Vec::new();
    if let Some(data) = value.get("data").and_then(Value::as_array) {
        for item in data {
            if let Some(id) = item.get("id").and_then(Value::as_u64) {
                if id > 0 {
                    ids.push(id);
                }
            }
        }
    }

    if ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut friends: Vec<FriendInfo> = ids
        .iter()
        .map(|&id| FriendInfo {
            id,
            username: String::new(),
            display_name: String::new(),
            is_online: false,
            presence_type: 0,
            last_location: None,
            place_id: None,
        })
        .collect();

    for chunk in ids.chunks(50) {
        let payload = serde_json::json!({
            "userIds": chunk,
            "excludeBannedUsers": false
        });
        let mut user_req = agent().post("https://users.roblox.com/v1/users");
        if !clean.is_empty() {
            user_req = user_req.header("Cookie", &cookie_header);
        }
        if let Ok(mut res) = user_req
            .header("Content-Type", "application/json")
            .send(payload.to_string().as_bytes())
        {
            let mut body = String::new();
            if res
                .body_mut()
                .as_reader()
                .take(LIMIT as u64)
                .read_to_string(&mut body)
                .is_ok()
            {
                if let Ok(val) = serde_json::from_str::<Value>(&body) {
                    if let Some(arr) = val.get("data").and_then(Value::as_array) {
                        for item in arr {
                            let Some(id) = item.get("id").and_then(Value::as_u64) else {
                                continue;
                            };
                            let name = item
                                .get("name")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .to_string();
                            let display = item
                                .get("displayName")
                                .and_then(Value::as_str)
                                .unwrap_or(&name)
                                .to_string();
                            if let Some(f) = friends.iter_mut().find(|f| f.id == id) {
                                f.username = name;
                                f.display_name = display;
                            }
                        }
                    }
                }
            }
        }
    }

    for chunk in ids.chunks(25) {
        let presence_payload = serde_json::json!({
            "userIds": chunk
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

    friends.retain(|f| !f.username.is_empty() || !f.display_name.is_empty());
    friends.sort_by(|a, b| {
        b.presence_type.cmp(&a.presence_type).then_with(|| {
            a.display_name
                .to_lowercase()
                .cmp(&b.display_name.to_lowercase())
        })
    });
    Ok(friends)
}

pub fn fetch_authentication_ticket(cookie: &str) -> Result<String> {
    let clean = sanitize_cookie(cookie);
    if clean.is_empty() {
        return Err(Error::invalid("Empty cookie"));
    }
    let cookie_header = format!(".ROBLOSECURITY={clean}");

    let mut res = raw_agent()
        .post("https://auth.roblox.com/v1/authentication-ticket")
        .header("Cookie", &cookie_header)
        .header("Origin", "https://www.roblox.com")
        .header("Referer", "https://www.roblox.com/")
        .header("RBX-Authentication-Negotiation", "1")
        .header("Content-Type", "application/json")
        .send(b"{}")
        .map_err(|err| Error::invalid(format!("Could not get auth ticket: {err}")))?;

    if res.status().as_u16() == 403 {
        if let Some(csrf) = res
            .headers()
            .get("x-csrf-token")
            .and_then(|v| v.to_str().ok())
        {
            res = raw_agent()
                .post("https://auth.roblox.com/v1/authentication-ticket")
                .header("Cookie", &cookie_header)
                .header("Origin", "https://www.roblox.com")
                .header("Referer", "https://www.roblox.com/")
                .header("RBX-Authentication-Negotiation", "1")
                .header("x-csrf-token", csrf)
                .header("Content-Type", "application/json")
                .send(b"{}")
                .map_err(|err| Error::invalid(format!("Could not get auth ticket: {err}")))?;
        }
    }

    if let Some(val) = res.headers().get("rbx-authentication-ticket") {
        if let Ok(ticket) = val.to_str() {
            if !ticket.is_empty() {
                return Ok(ticket.to_string());
            }
        }
    }

    Err(Error::invalid("No authentication ticket returned"))
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
        assert_eq!(
            sanitize_cookie(".ROBLOSECURITY=abc; path=/; domain=.roblox.com"),
            "abc"
        );
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
