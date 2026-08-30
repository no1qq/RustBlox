use std::io::Read;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};
use crate::util::fs;

const TIMEOUT: Duration = Duration::from_secs(10);
const LIMIT: usize = 64 * 1024;

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
