use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use chrono::Local;

struct Sink {
    file: Option<std::fs::File>,
    lines: Vec<Entry>,
    to_file: bool,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub at: chrono::DateTime<Local>,
    pub level: Level,
    pub message: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
    Error,
}

impl Level {
    pub fn label(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

const MAX_RETAINED: usize = 500;

static SINK: OnceLock<Mutex<Sink>> = OnceLock::new();

fn sink() -> &'static Mutex<Sink> {
    SINK.get_or_init(|| {
        Mutex::new(Sink {
            file: None,
            lines: Vec::new(),
            to_file: true,
        })
    })
}

pub fn attach(path: PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();
    if let Ok(mut guard) = sink().lock() {
        guard.file = file;
    }
    write(Level::Info, format!("log opened at {}", path.display()));
}

pub fn write(level: Level, message: impl Into<String>) {
    let entry = Entry {
        at: Local::now(),
        level,
        message: message.into(),
    };
    let Ok(mut guard) = sink().lock() else {
        return;
    };
    let to_file = guard.to_file;
    if let Some(file) = guard.file.as_mut().filter(|_| to_file) {
        let _ = writeln!(
            file,
            "{} [{}] {}",
            entry.at.format("%Y-%m-%d %H:%M:%S%.3f"),
            entry.level.label(),
            entry.message
        );
    }
    if guard.lines.len() >= MAX_RETAINED {
        guard.lines.remove(0);
    }
    guard.lines.push(entry);
}

pub fn recent(limit: usize) -> Vec<Entry> {
    let Ok(guard) = sink().lock() else {
        return Vec::new();
    };
    let start = guard.lines.len().saturating_sub(limit);
    guard.lines[start..].to_vec()
}

#[macro_export]
macro_rules! log_info {
    ($($arg:tt)*) => { $crate::util::log::write($crate::util::log::Level::Info, format!($($arg)*)) };
}

#[macro_export]
macro_rules! log_warn {
    ($($arg:tt)*) => { $crate::util::log::write($crate::util::log::Level::Warn, format!($($arg)*)) };
}

#[macro_export]
macro_rules! log_error {
    ($($arg:tt)*) => { $crate::util::log::write($crate::util::log::Level::Error, format!($($arg)*)) };
}

pub fn set_file_logging(enabled: bool) {
    if let Ok(mut guard) = sink().lock() {
        guard.to_file = enabled;
    }
}
