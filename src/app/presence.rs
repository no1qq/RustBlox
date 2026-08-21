use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::discord::{Connection, Details};
use crate::{log_info, log_warn};

const POLL: Duration = Duration::from_millis(400);
const RETRY: Duration = Duration::from_secs(15);

enum Command {
    Set(Box<Details>),
    Clear,
    Stop,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Off,
    Connecting,
    On,
    Failed(String),
}

impl Status {
    pub fn label(&self) -> String {
        match self {
            Status::Off => "Off".to_owned(),
            Status::Connecting => "Looking for Discord".to_owned(),
            Status::On => "Showing on your profile".to_owned(),
            Status::Failed(why) => why.clone(),
        }
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, Status::Failed(_))
    }
}

#[derive(Default)]
pub struct Presence {
    sender: Option<Sender<Command>>,
    status: Arc<Mutex<Status>>,
    shown: Option<Details>,
}

impl Presence {
    pub fn status(&self) -> Status {
        self.status
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }

    pub fn is_running(&self) -> bool {
        self.sender.is_some()
    }

    pub fn start(&mut self, application_id: String) {
        if self.is_running() {
            return;
        }

        let (sender, receiver) = channel();
        let status = Arc::clone(&self.status);
        set(&status, Status::Connecting);

        let builder = std::thread::Builder::new().name("rustblox-presence".into());
        let spawned = builder.spawn(move || {
            let mut connection: Option<Connection> = None;
            let mut wanted: Option<Details> = None;
            let mut next_try = Instant::now();

            loop {
                match receiver.recv_timeout(POLL) {
                    Ok(Command::Set(details)) => {
                        let details = *details;
                        if let Some(open) = connection.as_mut() {
                            if let Err(err) = open.set(&details) {
                                log_warn!("the Discord presence could not be updated: {err}");
                                connection = None;
                                set(&status, Status::Connecting);
                                next_try = Instant::now() + RETRY;
                            }
                        }
                        wanted = Some(details);
                    }
                    Ok(Command::Clear) => {
                        wanted = None;
                        if let Some(open) = connection.as_mut() {
                            let _ = open.clear();
                        }
                    }
                    Ok(Command::Stop) => break,
                    Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => {}
                }

                if connection.is_none() && Instant::now() >= next_try {
                    match Connection::open(&application_id) {
                        Ok(mut open) => {
                            log_info!("connected to Discord");
                            if let Some(details) = &wanted {
                                let _ = open.set(details);
                            }
                            connection = Some(open);
                            set(&status, Status::On);
                        }
                        Err(err) => {
                            set(&status, Status::Failed(short(&err.to_string())));
                            next_try = Instant::now() + RETRY;
                        }
                    }
                }
            }

            if let Some(mut open) = connection.take() {
                let _ = open.clear();
            }
            set(&status, Status::Off);
        });

        match spawned {
            Ok(_) => self.sender = Some(sender),
            Err(err) => {
                log_warn!("the Discord worker could not be started: {err}");
                set(&self.status, Status::Failed("could not start".into()));
            }
        }
    }

    pub fn stop(&mut self) {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(Command::Stop);
        }
        self.shown = None;
        set(&self.status, Status::Off);
    }

    pub fn show(&mut self, details: Details) {
        if self.shown.as_ref() == Some(&details) {
            return;
        }
        if let Some(sender) = &self.sender {
            if sender.send(Command::Set(Box::new(details.clone()))).is_ok() {
                self.shown = Some(details);
            }
        }
    }

    pub fn hide(&mut self) {
        if self.shown.is_none() {
            return;
        }
        self.shown = None;
        if let Some(sender) = &self.sender {
            let _ = sender.send(Command::Clear);
        }
    }
}

fn set(status: &Arc<Mutex<Status>>, next: Status) {
    if let Ok(mut held) = status.lock() {
        *held = next;
    }
}

fn short(message: &str) -> String {
    let first = message.split(':').next().unwrap_or(message).trim();
    if first.is_empty() {
        "Discord could not be reached".to_owned()
    } else {
        first.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_presence_is_off_and_not_running() {
        let presence = Presence::default();
        assert_eq!(presence.status(), Status::Off);
        assert!(!presence.is_running());
        assert_eq!(presence.status().label(), "Off");
    }

    #[test]
    fn showing_without_a_worker_keeps_nothing() {
        let mut presence = Presence::default();
        presence.show(Details {
            line: "Playing".into(),
            ..Details::default()
        });
        assert!(presence.shown.is_none());
    }

    #[test]
    fn a_failure_reads_as_failed_and_keeps_its_first_clause() {
        let status = Status::Failed(short("Discord is not running: something else"));
        assert!(status.is_failed());
        assert_eq!(status.label(), "Discord is not running");
    }

    #[test]
    fn stopping_a_presence_that_never_started_is_harmless() {
        let mut presence = Presence::default();
        presence.stop();
        assert_eq!(presence.status(), Status::Off);
    }
}
