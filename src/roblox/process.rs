use crate::platform::{self, ProcessInfo};

use super::install::PLAYER_EXE;

pub const STUDIO_EXE: &str = "RobloxStudioBeta.exe";
pub const CRASH_HANDLER_EXE: &str = "RobloxCrashHandler.exe";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RobloxStatus {
    pub players: Vec<ProcessInfo>,
    pub studios: Vec<ProcessInfo>,
}

impl RobloxStatus {
    pub fn player_running(&self) -> bool {
        !self.players.is_empty()
    }

    pub fn summary(&self) -> String {
        match (self.players.len(), self.studios.len()) {
            (0, 0) => "Not running".into(),
            (1, 0) => "Running".into(),
            (n, 0) => format!("{n} clients running"),
            (0, 1) => "Studio running".into(),
            (0, n) => format!("{n} Studio windows"),
            (p, s) => format!("{p} clients, {s} Studio windows"),
        }
    }
}

pub fn status() -> RobloxStatus {
    let found = platform::find_processes(&[PLAYER_EXE, STUDIO_EXE]);
    let mut status = RobloxStatus::default();
    for process in found {
        if process.name.eq_ignore_ascii_case(STUDIO_EXE) {
            status.studios.push(process);
        } else {
            status.players.push(process);
        }
    }
    status
}

pub fn is_pid_alive(pid: u32) -> bool {
    platform::find_processes(&[PLAYER_EXE, STUDIO_EXE, CRASH_HANDLER_EXE])
        .iter()
        .any(|process| process.pid == pid)
}
