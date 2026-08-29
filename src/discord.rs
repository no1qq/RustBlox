use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::error::{Error, Result};

pub const DEFAULT_APPLICATION_ID: &str = "1479505409331953827";

const PIPES: u8 = 10;
const MAX_FRAME: u32 = 64 * 1024;

const OP_HANDSHAKE: u32 = 0;
const OP_FRAME: u32 = 1;
const OP_CLOSE: u32 = 2;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Details {
    pub line: String,
    pub note: String,
    pub started_at: Option<u64>,
}

impl Details {
    pub fn as_activity(&self) -> Value {
        let mut activity = json!({});
        if !self.line.trim().is_empty() {
            activity["details"] = json!(trim_to(&self.line, 128));
        }
        if !self.note.trim().is_empty() {
            activity["state"] = json!(trim_to(&self.note, 128));
        }
        if let Some(started) = self.started_at {
            activity["timestamps"] = json!({ "start": started });
        }
        activity
    }
}

fn trim_to(text: &str, limit: usize) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= limit {
        return trimmed.to_owned();
    }
    trimmed.chars().take(limit).collect()
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

pub fn looks_like_application_id(id: &str) -> bool {
    let trimmed = id.trim();
    (17..=20).contains(&trimmed.len()) && trimmed.chars().all(|c| c.is_ascii_digit())
}

pub struct Connection {
    pipe: File,
    nonce: u64,
}

impl Connection {
    pub fn open(application_id: &str) -> Result<Self> {
        if !looks_like_application_id(application_id) {
            return Err(Error::invalid(
                "a Discord application ID is 17 to 20 digits",
            ));
        }

        let mut handshake_err = None;
        let mut opened_any = false;

        for index in 0..PIPES {
            if let Ok(pipe) = open_pipe(index) {
                opened_any = true;
                let mut connection = Self { pipe, nonce: 0 };
                match connection.handshake(application_id.trim()) {
                    Ok(()) => return Ok(connection),
                    Err(err) => handshake_err = Some(err),
                }
            }
        }

        if let Some(err) = handshake_err {
            return Err(err);
        }

        if !opened_any {
            return Err(Error::invalid("Discord is not running"));
        }

        Err(Error::invalid("Discord is not reachable"))
    }

    fn handshake(&mut self, application_id: &str) -> Result<()> {
        let hello = json!({ "v": 1, "client_id": application_id });
        self.send(OP_HANDSHAKE, &hello)?;

        let (opcode, body) = self.receive()?;
        if opcode == OP_CLOSE {
            return Err(Error::invalid(format!(
                "Discord refused the connection: {}",
                reason(&body)
            )));
        }
        Ok(())
    }

    pub fn set(&mut self, details: &Details) -> Result<()> {
        self.command(details.as_activity())
    }

    pub fn clear(&mut self) -> Result<()> {
        self.command(Value::Null)
    }

    fn command(&mut self, activity: Value) -> Result<()> {
        self.nonce += 1;
        let payload = json!({
            "cmd": "SET_ACTIVITY",
            "nonce": self.nonce.to_string(),
            "args": {
                "pid": std::process::id(),
                "activity": activity,
            },
        });
        self.send(OP_FRAME, &payload)?;
        let (opcode, body) = self.receive()?;
        if opcode == OP_CLOSE {
            return Err(Error::invalid(format!(
                "Discord closed the connection: {}",
                reason(&body)
            )));
        }
        Ok(())
    }

    fn send(&mut self, opcode: u32, payload: &Value) -> Result<()> {
        let body = serde_json::to_vec(payload).unwrap_or_default();
        let frame = frame(opcode, &body);
        self.pipe
            .write_all(&frame)
            .map_err(|err| Error::io("could not write to the Discord pipe", err))?;
        self.pipe
            .flush()
            .map_err(|err| Error::io("could not flush the Discord pipe", err))
    }

    fn receive(&mut self) -> Result<(u32, Vec<u8>)> {
        let mut header = [0_u8; 8];
        self.pipe
            .read_exact(&mut header)
            .map_err(|err| Error::io("Discord stopped answering", err))?;

        let (opcode, length) = split_header(&header);
        if length > MAX_FRAME {
            return Err(Error::invalid("Discord sent an unreasonably large frame"));
        }

        let mut body = vec![0_u8; length as usize];
        self.pipe
            .read_exact(&mut body)
            .map_err(|err| Error::io("Discord stopped answering", err))?;
        Ok((opcode, body))
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.pipe.write_all(&frame(OP_CLOSE, b"{}"));
        let _ = self.pipe.flush();
    }
}

fn frame(opcode: u32, body: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&opcode.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(body);
    out
}

fn split_header(header: &[u8; 8]) -> (u32, u32) {
    let opcode = u32::from_le_bytes([header[0], header[1], header[2], header[3]]);
    let length = u32::from_le_bytes([header[4], header[5], header[6], header[7]]);
    (opcode, length)
}

fn reason(body: &[u8]) -> String {
    serde_json::from_slice::<Value>(body)
        .ok()
        .and_then(|value| {
            value
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "no reason given".to_owned())
}

#[cfg(windows)]
fn open_pipe(index: u8) -> Result<File> {
    let path = format!(r"\\.\pipe\discord-ipc-{index}");
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
        .map_err(|err| Error::io(format!("{path} could not be opened"), err))
}

#[cfg(not(windows))]
fn open_pipe(_index: u8) -> Result<File> {
    Err(Error::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_frame_carries_its_opcode_and_length_first() {
        let bytes = frame(OP_FRAME, b"hello");

        assert_eq!(&bytes[0..4], &1_u32.to_le_bytes());
        assert_eq!(&bytes[4..8], &5_u32.to_le_bytes());
        assert_eq!(&bytes[8..], b"hello");
    }

    #[test]
    fn a_header_reads_back_the_way_it_was_written() {
        let bytes = frame(OP_HANDSHAKE, b"{}");
        let header: [u8; 8] = bytes[..8].try_into().unwrap();

        assert_eq!(split_header(&header), (OP_HANDSHAKE, 2));
    }

    #[test]
    fn an_empty_body_is_still_a_valid_frame() {
        let bytes = frame(OP_CLOSE, b"");
        assert_eq!(bytes.len(), 8);
        assert_eq!(split_header(&bytes[..8].try_into().unwrap()), (OP_CLOSE, 0));
    }

    #[test]
    fn an_activity_only_carries_the_lines_that_have_something_in_them() {
        let details = Details {
            line: "Playing something".into(),
            note: String::new(),
            started_at: Some(1787317890),
        };

        let activity = details.as_activity();

        assert_eq!(activity["details"], json!("Playing something"));
        assert!(activity.get("state").is_none());
        assert_eq!(activity["timestamps"]["start"], json!(1787317890));
    }

    #[test]
    fn an_empty_activity_is_an_empty_object() {
        assert_eq!(Details::default().as_activity(), json!({}));
    }

    #[test]
    fn long_lines_are_cut_to_what_discord_takes() {
        let details = Details {
            line: "a".repeat(400),
            note: "b".repeat(400),
            started_at: None,
        };

        let activity = details.as_activity();

        assert_eq!(activity["details"].as_str().unwrap().len(), 128);
        assert_eq!(activity["state"].as_str().unwrap().len(), 128);
    }

    #[test]
    fn an_application_id_has_to_look_like_a_snowflake() {
        assert!(looks_like_application_id("123456789012345678"));
        assert!(looks_like_application_id("  123456789012345678  "));
        assert!(!looks_like_application_id(""));
        assert!(!looks_like_application_id("12345"));
        assert!(!looks_like_application_id("not-a-number-at-all"));
        assert!(!looks_like_application_id(&"1".repeat(30)));
    }

    #[test]
    fn a_bad_application_id_never_reaches_the_pipe() {
        match Connection::open("nonsense") {
            Ok(_) => panic!("nonsense was accepted as an application ID"),
            Err(err) => assert!(err.to_string().contains("17 to 20 digits")),
        }
    }

    #[test]
    fn the_application_it_ships_with_is_one_discord_would_take() {
        assert!(looks_like_application_id(DEFAULT_APPLICATION_ID));
    }

    #[test]
    #[ignore = "needs Discord running"]
    fn a_real_presence_reaches_discord() {
        let id = std::env::var("RUSTBLOX_DISCORD_APP_ID")
            .unwrap_or_else(|_| DEFAULT_APPLICATION_ID.to_owned());

        let mut connection = Connection::open(&id).expect("Discord did not accept the handshake");
        connection
            .set(&Details {
                line: "Playing Some Game".into(),
                note: "Place 14705961406".into(),
                started_at: Some(now()),
            })
            .expect("the activity was refused");

        std::thread::sleep(std::time::Duration::from_secs(6));
        connection
            .clear()
            .expect("the activity could not be cleared");
    }

    #[test]
    fn a_refusal_reads_the_reason_discord_gave() {
        let body = br#"{"code":4000,"message":"Invalid Client ID"}"#;
        assert_eq!(reason(body), "Invalid Client ID");
        assert_eq!(reason(b"not json"), "no reason given");
    }
}
