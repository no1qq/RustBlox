use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::error::{Error, Result};
use crate::platform;

use super::detect;
use super::flags::{self, FlagProfile};
use super::gamesettings::{self, Change};
use super::mods;
use super::process;
use super::uri;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LaunchTarget {
    App,
    Place {
        place_id: u64,
        label: Option<String>,
    },
    Forward(String),
}

impl LaunchTarget {
    pub fn headline(&self) -> String {
        match self {
            LaunchTarget::App => "Roblox home".into(),
            LaunchTarget::Place { place_id, label } => match label {
                Some(label) if !label.trim().is_empty() => label.clone(),
                _ => format!("Place {place_id}"),
            },
            LaunchTarget::Forward(value) => uri::summarise(value).headline(),
        }
    }

    pub fn detail(&self) -> String {
        match self {
            LaunchTarget::App => "Opens the Roblox client on its own home screen.".into(),
            LaunchTarget::Place { place_id, .. } => {
                format!("Opens the Roblox client on place {place_id}.")
            }
            LaunchTarget::Forward(_) => {
                "Hands a launch link from your browser straight to the Roblox client.".into()
            }
        }
    }

    pub fn arguments(&self) -> Result<Vec<String>> {
        Ok(match self {
            LaunchTarget::App => vec!["--app".into()],
            LaunchTarget::Place { place_id, .. } => vec![uri::deep_link(*place_id)],
            LaunchTarget::Forward(value) => vec![uri::validate(value)?],
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StepId {
    Prepare,
    Locate,
    Verify,
    Configure,
    Start,
    Confirm,
}

impl StepId {
    pub const ORDER: [StepId; 6] = [
        StepId::Prepare,
        StepId::Locate,
        StepId::Verify,
        StepId::Configure,
        StepId::Start,
        StepId::Confirm,
    ];

    pub fn title(self) -> &'static str {
        match self {
            StepId::Prepare => "Preparing",
            StepId::Locate => "Locating Roblox",
            StepId::Verify => "Checking the install",
            StepId::Configure => "Applying configuration",
            StepId::Start => "Starting the client",
            StepId::Confirm => "Waiting for Roblox",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Active,
    Done,
    Skipped,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub id: StepId,
    pub state: StepState,
    pub detail: Option<String>,
}

impl Step {
    pub fn pending(id: StepId) -> Self {
        Self {
            id,
            state: StepState::Pending,
            detail: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum LaunchEvent {
    Step {
        id: StepId,
        state: StepState,
        detail: Option<String>,
    },
    Finished(std::result::Result<LaunchReport, LaunchFailure>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchReport {
    pub pid: Option<u32>,
    pub confirmed: bool,
    pub version: Option<String>,
    pub target: String,
    pub elapsed: Duration,
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchFailure {
    pub step: StepId,
    pub message: String,
    pub hint: Option<String>,
    pub cancelled: bool,
}

impl LaunchFailure {
    fn from_error(step: StepId, error: &Error) -> Self {
        Self {
            step,
            message: error.to_string(),
            hint: error.hint().map(str::to_owned),
            cancelled: false,
        }
    }

    fn cancelled(step: StepId) -> Self {
        Self {
            step,
            message: "The launch was cancelled.".into(),
            hint: None,
            cancelled: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GamePlan {
    pub path: PathBuf,
    pub changes: Vec<Change>,
    pub lock: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModPlan {
    pub root: PathBuf,
    pub originals: PathBuf,
    pub enabled: bool,
}

#[derive(Clone, Debug)]
pub struct LaunchPlan {
    pub target: LaunchTarget,
    pub scan: detect::ScanOptions,
    pub verify: bool,
    pub flag_profile: Option<FlagProfile>,
    pub game: Option<GamePlan>,
    pub mods: Option<ModPlan>,
    pub backup_dir: PathBuf,
    pub extra_arguments: String,
    pub timeout: Duration,
    pub allow_when_running: bool,
    pub account_cookie: Option<String>,
}

pub struct Launcher;

impl Launcher {
    pub fn run(plan: LaunchPlan, cancel: Arc<AtomicBool>, emit: &dyn Fn(LaunchEvent)) {
        let started = Instant::now();
        match execute(&plan, &cancel, emit, started) {
            Ok(report) => emit(LaunchEvent::Finished(Ok(report))),
            Err(failure) => emit(LaunchEvent::Finished(Err(failure))),
        }
    }
}

fn active(emit: &dyn Fn(LaunchEvent), id: StepId) {
    emit(LaunchEvent::Step {
        id,
        state: StepState::Active,
        detail: None,
    });
}

fn done(emit: &dyn Fn(LaunchEvent), id: StepId, detail: impl Into<String>) {
    emit(LaunchEvent::Step {
        id,
        state: StepState::Done,
        detail: Some(detail.into()),
    });
}

fn skipped(emit: &dyn Fn(LaunchEvent), id: StepId, detail: impl Into<String>) {
    emit(LaunchEvent::Step {
        id,
        state: StepState::Skipped,
        detail: Some(detail.into()),
    });
}

fn failed(emit: &dyn Fn(LaunchEvent), id: StepId, detail: impl Into<String>) {
    emit(LaunchEvent::Step {
        id,
        state: StepState::Failed,
        detail: Some(detail.into()),
    });
}

fn check_cancel(cancel: &AtomicBool, step: StepId) -> std::result::Result<(), LaunchFailure> {
    if cancel.load(Ordering::Relaxed) {
        Err(LaunchFailure::cancelled(step))
    } else {
        Ok(())
    }
}

fn execute(
    plan: &LaunchPlan,
    cancel: &AtomicBool,
    emit: &dyn Fn(LaunchEvent),
    started: Instant,
) -> std::result::Result<LaunchReport, LaunchFailure> {
    active(emit, StepId::Prepare);
    let arguments = match plan.target.arguments() {
        Ok(arguments) => arguments,
        Err(err) => {
            failed(emit, StepId::Prepare, err.to_string());
            return Err(LaunchFailure::from_error(StepId::Prepare, &err));
        }
    };

    let running = process::status();
    if running.player_running() && !plan.allow_when_running {
        let message = "Roblox is already running.".to_string();
        failed(emit, StepId::Prepare, message.clone());
        return Err(LaunchFailure {
            step: StepId::Prepare,
            message,
            hint: Some(
                "Close the running client first, or allow extra clients in Launch settings.".into(),
            ),
            cancelled: false,
        });
    }
    done(
        emit,
        StepId::Prepare,
        if running.player_running() {
            "another client is already running".to_string()
        } else {
            plan.target.headline()
        },
    );
    check_cancel(cancel, StepId::Locate)?;

    active(emit, StepId::Locate);
    let detection = detect::scan(&plan.scan);
    let install = match detection.active() {
        Some(install) => install.clone(),
        None => {
            let err = Error::RobloxNotFound;
            failed(emit, StepId::Locate, err.to_string());
            return Err(LaunchFailure::from_error(StepId::Locate, &err));
        }
    };
    done(
        emit,
        StepId::Locate,
        format!("{} ({})", install.display_version(), install.source.short()),
    );
    check_cancel(cancel, StepId::Verify)?;

    active(emit, StepId::Verify);
    if !install.player.is_file() {
        let err = Error::PlayerMissing(install.version_dir.clone());
        failed(emit, StepId::Verify, err.to_string());
        return Err(LaunchFailure::from_error(StepId::Verify, &err));
    }
    if plan.verify {
        let integrity = install.integrity();
        if !integrity.is_ok() {
            let joined = integrity.problems.join("; ");
            failed(emit, StepId::Verify, joined.clone());
            return Err(LaunchFailure {
                step: StepId::Verify,
                message: format!("The install looks incomplete: {joined}"),
                hint: Some(
                    "Reinstall Roblox, or turn off install verification in Advanced settings."
                        .into(),
                ),
                cancelled: false,
            });
        }
        done(emit, StepId::Verify, "all expected files are present");
    } else {
        skipped(emit, StepId::Verify, "verification is turned off");
    }
    check_cancel(cancel, StepId::Configure)?;

    active(emit, StepId::Configure);
    let mut notes: Vec<String> = Vec::new();
    match &plan.flag_profile {
        Some(profile) => match flags::apply_to(&install, profile, &plan.backup_dir) {
            Ok(report) => notes.push(format!("wrote {} flags", report.count)),
            Err(err) => {
                failed(emit, StepId::Configure, err.to_string());
                return Err(LaunchFailure::from_error(StepId::Configure, &err));
            }
        },
        None => notes.push("no flag profile is applied".into()),
    }
    if let Some(game) = &plan.game {
        match gamesettings::apply(&game.path, &game.changes, game.lock, &plan.backup_dir) {
            Ok(report) => notes.push(report.summary()),
            Err(err) => notes.push(format!("game settings were left alone: {err}")),
        }
    }
    if let Some(plan) = &plan.mods {
        let originals = plan.originals.join(&install.folder_id);
        mods::forget_other_versions(&plan.originals, &install.folder_id);
        let outcome = if plan.enabled {
            mods::apply(&install.version_dir, &plan.root, &originals).map(|report| report.summary())
        } else {
            mods::restore_all(&install.version_dir, &originals).map(|restored| match restored {
                0 => "no mods to remove".to_owned(),
                count => format!("put {count} original files back"),
            })
        };
        match outcome {
            Ok(note) => notes.push(note),
            Err(err) => notes.push(format!("mods were left alone: {err}")),
        }
    }
    let cleaned = platform::clean_roblox_dir_proxies(&install.version_dir);
    if !cleaned.is_empty() {
        notes.push(format!("removed {} rogue proxy files", cleaned.len()));
    }
    done(emit, StepId::Configure, notes.join(", "));
    check_cancel(cancel, StepId::Start)?;

    active(emit, StepId::Start);
    let mut all_arguments = arguments;
    if let Some(cookie) = &plan.account_cookie {
        let _ = super::account::apply_account_session(cookie);
        if let Ok(ticket) = super::account::fetch_authentication_ticket(cookie) {
            match &plan.target {
                LaunchTarget::App => {
                    all_arguments = vec!["--app".into(), "-t".into(), ticket];
                }
                LaunchTarget::Place { place_id, .. } => {
                    all_arguments = vec![
                        "--app".into(),
                        "-t".into(),
                        ticket,
                        "-j".into(),
                        format!(
                            "https://assetgame.roblox.com/game/PlaceLauncher.ashx?request=RequestGame&placeId={place_id}&isPlayTogetherGame=false"
                        ),
                    ];
                }
                LaunchTarget::Forward(_) => {
                    all_arguments.push("-t".into());
                    all_arguments.push(ticket);
                }
            }
        }
    }
    all_arguments.extend(split_arguments(&plan.extra_arguments));

    let mut child = match platform::spawn_detached(
        &install.player,
        &all_arguments,
        Some(install.version_dir.as_path()),
    ) {
        Ok(child) => child,
        Err(err) => {
            failed(emit, StepId::Start, err.to_string());
            return Err(LaunchFailure::from_error(StepId::Start, &err));
        }
    };
    let pid = child.id();
    platform::bind_launcher_job_roblox(pid);
    done(emit, StepId::Start, format!("process {pid} created"));

    active(emit, StepId::Confirm);
    match confirm(&mut child, pid, plan.timeout, cancel) {
        Confirmation::Running => {
            done(emit, StepId::Confirm, "the client is running");
            Ok(LaunchReport {
                pid: Some(pid),
                confirmed: true,
                version: install.version.clone(),
                target: plan.target.headline(),
                elapsed: started.elapsed(),
                note: None,
            })
        }
        Confirmation::HandedOff => {
            done(emit, StepId::Confirm, "handed over to an existing client");
            Ok(LaunchReport {
                pid: None,
                confirmed: true,
                version: install.version.clone(),
                target: plan.target.headline(),
                elapsed: started.elapsed(),
                note: Some(
                    "Roblox passed the request to a client that was already running.".into(),
                ),
            })
        }
        Confirmation::Timeout => {
            done(emit, StepId::Confirm, "no client reported yet");
            Ok(LaunchReport {
                pid: Some(pid),
                confirmed: false,
                version: install.version.clone(),
                target: plan.target.headline(),
                elapsed: started.elapsed(),
                note: Some(format!(
                    "Roblox was started but had not reported a running client after {}s. It may still be loading.",
                    plan.timeout.as_secs()
                )),
            })
        }
        Confirmation::Exited(code) => {
            let message = match code {
                Some(code) => format!("Roblox exited immediately with code {code}."),
                None => "Roblox exited immediately.".to_string(),
            };
            failed(emit, StepId::Confirm, message.clone());
            Err(LaunchFailure {
                step: StepId::Confirm,
                message,
                hint: Some(
                    "This usually means the install is damaged or the launch link was rejected. Try a rescan from the Installation page."
                        .into(),
                ),
                cancelled: false,
            })
        }
        Confirmation::Cancelled => {
            failed(emit, StepId::Confirm, "cancelled");
            Err(LaunchFailure::cancelled(StepId::Confirm))
        }
    }
}

enum Confirmation {
    Running,
    HandedOff,
    Timeout,
    Exited(Option<i32>),
    Cancelled,
}

const POLL_INTERVAL: Duration = Duration::from_millis(200);
const HANDOFF_GRACE: Duration = Duration::from_millis(1500);

fn confirm(
    child: &mut std::process::Child,
    pid: u32,
    timeout: Duration,
    cancel: &AtomicBool,
) -> Confirmation {
    let deadline = Instant::now() + timeout;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Confirmation::Cancelled;
        }

        match child.try_wait() {
            Ok(Some(status)) => {
                std::thread::sleep(HANDOFF_GRACE);
                if process::status().player_running() {
                    return Confirmation::HandedOff;
                }
                return Confirmation::Exited(status.code());
            }
            Ok(None) => {
                if process::is_pid_alive(pid) {
                    return Confirmation::Running;
                }
            }
            Err(_) => {
                if process::status().player_running() {
                    return Confirmation::Running;
                }
            }
        }

        if Instant::now() >= deadline {
            return if process::status().player_running() {
                Confirmation::Running
            } else {
                Confirmation::Timeout
            };
        }

        std::thread::sleep(POLL_INTERVAL);
    }
}

pub fn split_arguments(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quoted = false;

    for character in input.chars() {
        match character {
            '"' => quoted = !quoted,
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }

    if !current.is_empty() {
        out.push(current);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_arguments_respecting_quotes() {
        assert!(split_arguments("   ").is_empty());
        assert_eq!(split_arguments("--a --b"), vec!["--a", "--b"]);
        assert_eq!(
            split_arguments(r#"--path "C:\Program Files\x" --flag"#),
            vec!["--path", r"C:\Program Files\x", "--flag"]
        );
    }

    #[test]
    fn app_target_uses_the_app_switch() {
        assert_eq!(LaunchTarget::App.arguments().unwrap(), vec!["--app"]);
    }

    #[test]
    fn place_target_uses_a_deep_link() {
        let target = LaunchTarget::Place {
            place_id: 1818,
            label: None,
        };
        assert_eq!(
            target.arguments().unwrap(),
            vec!["roblox://experiences/start?placeId=1818"]
        );
        assert_eq!(target.headline(), "Place 1818");
    }

    #[test]
    fn forward_target_validates_before_launching() {
        let good = LaunchTarget::Forward("roblox-player:1+launchmode:app".into());
        assert_eq!(
            good.arguments().unwrap(),
            vec!["roblox-player:1+launchmode:app"]
        );

        let bad = LaunchTarget::Forward("https://example.com".into());
        assert!(bad.arguments().is_err());
    }

    #[test]
    fn labelled_place_prefers_its_name() {
        let target = LaunchTarget::Place {
            place_id: 5,
            label: Some("Weekend server".into()),
        };
        assert_eq!(target.headline(), "Weekend server");
    }
}
