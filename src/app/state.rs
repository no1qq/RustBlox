use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::Local;

use crate::cli::Invocation;
use crate::config::{LaunchOutcome, LaunchRecord, Settings, StartupTarget, State, Store};
use crate::error::Result;
use crate::platform::{self, SchemeRegistration};
use crate::roblox::activity::{self, Activity};
use crate::roblox::deploy::Deployment;
use crate::roblox::detect::{Detection, ScanOptions};
use crate::roblox::flags::{self, FlagProfile};
use crate::roblox::gamesettings::{self, Snapshot};
use crate::roblox::installer::InstallPlan;
use crate::roblox::launch::{GamePlan, LaunchPlan, LaunchTarget, ModPlan};
use crate::roblox::mods;
use crate::roblox::process::RobloxStatus;
use crate::roblox::versions;
use crate::shortcuts;
use crate::{log_error, log_info, log_warn};

use super::flow::{FlowStage, FlowStatus, LaunchFlow};
use super::install_session::{InstallPhase, InstallSession};
use super::selfupdate::AppUpdate;
use super::session::{LaunchSession, Phase};
use super::tasks::{AppDownload, Tasks, Update};
use super::toast::{ToastKind, Toasts};

pub use crate::roblox::uri::{SCHEME_DEEPLINK as DEEPLINK_SCHEME, SCHEME_PLAYER as PLAYER_SCHEME};

const IDLE_POLL: Duration = Duration::from_millis(2500);
const BUSY_POLL: Duration = Duration::from_millis(900);
const SAVE_DEBOUNCE: Duration = Duration::from_millis(600);
const THEME_PROBE: Duration = Duration::from_millis(900);
const DENIED_PROBE: Duration = Duration::from_secs(5);
const ACTIVITY_PROBE: Duration = Duration::from_secs(3);

pub struct AppState {
    pub store: Store,
    pub settings: Settings,
    pub persisted: State,
    pub detection: Detection,
    pub latest: Option<Deployment>,
    pub latest_note: Option<String>,
    pub roblox: RobloxStatus,
    pub activity: Activity,
    pub place_names: std::collections::HashMap<u64, String>,
    pub flags: FlagProfile,
    pub denied_flags: Vec<String>,
    pub game: Snapshot,
    pub mods: mods::Inventory,
    pub shortcuts: shortcuts::Present,
    pub session: LaunchSession,
    pub install: InstallSession,
    pub flow: LaunchFlow,
    pub app_update: AppUpdate,
    pub tasks: Tasks,
    pub toasts: Toasts,
    pub protocol: Option<SchemeRegistration>,
    pub deeplink: Option<SchemeRegistration>,
    pub system_dark: bool,
    pub startup_notes: Vec<String>,
    pub exe_path: Option<PathBuf>,
    pub close_requested: bool,
    pub security: crate::roblox::security::SecurityWatchdog,
    pub accounts: Vec<crate::roblox::account::AccountProfile>,
    pub active_account_id: Option<u64>,

    settings_dirty_at: Option<Instant>,
    flags_dirty_at: Option<Instant>,
    game_dirty_at: Option<Instant>,
    denied_checked_at: Option<Instant>,
    game_checked_at: Option<Instant>,
    mods_checked_at: Option<Instant>,
    shortcuts_checked_at: Option<Instant>,
    activity_checked_at: Option<Instant>,
    activity_since: Option<u64>,
    presence: super::presence::Presence,
    client_seen: bool,
    state_dirty: bool,
    last_poll: Instant,
    last_theme_probe: Instant,
    initial_scan_done: bool,
}

impl AppState {
    pub fn new(store: Store, invocation: &Invocation) -> Self {
        let mut startup_notes = Vec::new();

        let loaded_settings = store.load_settings();
        for note in &loaded_settings.notes {
            log_warn!("settings: {note}");
        }
        startup_notes.extend(loaded_settings.notes);
        if loaded_settings.recovered {
            startup_notes.push("settings were rebuilt from defaults after a problem".into());
        }

        let settings = if invocation.reset {
            if let Some(saved) = crate::util::fs::quarantine(&store.paths().settings_file()) {
                startup_notes.push(format!("previous settings kept at {}", saved.display()));
            }
            startup_notes.push("started with default settings".into());
            Settings::default()
        } else {
            loaded_settings.value
        };

        let loaded_state = store.load_state();
        for note in &loaded_state.notes {
            log_warn!("state: {note}");
        }
        startup_notes.extend(loaded_state.notes);
        if loaded_state.recovered {
            startup_notes.push("saved state was rebuilt from defaults after a problem".into());
        }

        let active_profile = settings.advanced.active_flag_profile.clone();
        let flags =
            match flags::load_named_profile(&store.paths().flag_profiles_dir(), &active_profile) {
                Ok(profile) => profile,
                Err(err) => {
                    log_error!("flag profile could not be read: {err}");
                    startup_notes.push(format!("flag profile could not be read: {err}"));
                    FlagProfile::default()
                }
            };

        let accounts = crate::roblox::account::load_accounts(&store.paths().accounts_file())
            .unwrap_or_default();
        let active_account_id = accounts.first().map(|acc| acc.id);

        let mut app = Self {
            store,
            settings,
            persisted: loaded_state.value,
            detection: Detection::default(),
            latest: None,
            latest_note: None,
            roblox: RobloxStatus::default(),
            activity: Activity::default(),
            place_names: std::collections::HashMap::new(),
            flags,
            denied_flags: Vec::new(),
            game: Snapshot::default(),
            mods: mods::Inventory::default(),
            shortcuts: shortcuts::Present::default(),
            session: LaunchSession::default(),
            install: InstallSession::default(),
            flow: LaunchFlow::default(),
            app_update: AppUpdate::default(),
            tasks: Tasks::default(),
            toasts: Toasts::default(),
            protocol: None,
            deeplink: None,
            system_dark: platform::system_dark_mode().unwrap_or(true),
            startup_notes,
            exe_path: std::env::current_exe().ok(),
            close_requested: false,
            security: crate::roblox::security::SecurityWatchdog::default(),
            accounts,
            active_account_id,
            settings_dirty_at: None,
            flags_dirty_at: None,
            game_dirty_at: None,
            denied_checked_at: None,
            game_checked_at: None,
            mods_checked_at: None,
            shortcuts_checked_at: None,
            activity_checked_at: None,
            activity_since: None,
            presence: super::presence::Presence::default(),
            client_seen: false,
            state_dirty: false,
            last_poll: Instant::now() - IDLE_POLL,
            last_theme_probe: Instant::now() - THEME_PROBE,
            initial_scan_done: false,
        };

        app.refresh_protocol();
        if let Some(exe) = app.exe_path.clone() {
            crate::selfupdate::clear_retired(&exe);
        }
        app.refresh_shortcuts(true);
        app.check_auto_clean_cache();
        app
    }

    pub fn attach(&mut self, ctx: egui::Context) {
        self.tasks.attach(ctx);
    }

    pub fn tick(&mut self) {
        if !self.initial_scan_done {
            self.initial_scan_done = true;
            self.rescan();
            self.check_app_update();
        }

        for update in self.tasks.drain() {
            self.consume(update);
        }

        let interval = if self.session.phase.is_busy() || self.flow.stage.is_busy() {
            BUSY_POLL
        } else {
            IDLE_POLL
        };
        if self.last_poll.elapsed() >= interval {
            self.last_poll = Instant::now();
            self.tasks.poll_processes();
        }

        if let Some(since) = self.settings_dirty_at {
            if since.elapsed() >= SAVE_DEBOUNCE {
                self.flush_settings();
            }
        }

        if let Some(since) = self.flags_dirty_at {
            if since.elapsed() >= SAVE_DEBOUNCE {
                self.flush_flags();
            }
        }

        if let Some(since) = self.game_dirty_at {
            if since.elapsed() >= SAVE_DEBOUNCE {
                self.flush_game_settings();
            }
        }

        if self.last_theme_probe.elapsed() >= THEME_PROBE {
            self.last_theme_probe = Instant::now();
            if let Some(dark) = platform::system_dark_mode() {
                self.system_dark = dark;
            }
        }

        self.refresh_activity();
        self.toasts.retire_expired();
    }

    fn consume(&mut self, update: Update) {
        match update {
            Update::Scanned(detection) => {
                let previous = self
                    .detection
                    .active()
                    .map(|install| install.folder_id.clone());
                self.detection = *detection;

                for note in &self.detection.notes {
                    log_info!("scan: {note}");
                }

                match self.detection.active() {
                    Some(install) => {
                        if previous.as_deref() != Some(install.folder_id.as_str()) {
                            log_info!(
                                "using Roblox {} at {}",
                                install.display_version(),
                                install.version_dir.display()
                            );
                        }
                    }
                    None => log_warn!("no Roblox installation was found"),
                }
            }
            Update::Processes(status) => {
                let was_running = self.roblox.player_running();
                self.roblox = status;
                if self.roblox.player_running() {
                    self.client_seen = true;
                    if !self.security.is_running() {
                        if let Some(player) = self.roblox.players.first() {
                            let install_dir =
                                self.detection.active().map(|i| i.version_dir.clone());
                            self.security.start(player.pid, install_dir);
                        }
                    }
                } else if was_running {
                    log_info!("the Roblox client closed");
                    self.security.stop();
                    if self.flow.stage == FlowStage::Watching {
                        self.flow.stage = FlowStage::Finished;
                        if self.settings.launch.close_after_launch {
                            self.close_requested = true;
                        }
                    }
                }
            }
            Update::Launch(event) => {
                self.session.apply(event);
                if self.session.phase.is_finished() {
                    self.finish_launch();
                }
            }
            Update::Install(event) => {
                self.install.apply(event);
                if self.install.phase.is_finished() {
                    self.finish_install();
                }
            }
            Update::Latest(found) => match *found {
                Ok(deployment) => {
                    log_info!(
                        "the {} channel is on {}",
                        deployment.channel,
                        deployment.version
                    );
                    self.latest_note = None;
                    self.latest = Some(deployment);
                }
                Err(message) => {
                    log_warn!("the latest version could not be checked: {message}");
                    self.latest_note = Some(message);
                }
            },
            Update::AppRelease(found) => match *found {
                Ok(Some(release)) => {
                    log_info!("RustBlox {} is available", release.version);
                    let rel_clone = release.clone();
                    self.app_update.found(Some(release));
                    if let Some(exe) = self.exe_path.clone() {
                        self.tasks.download_app_update(rel_clone, exe);
                    }
                }
                Ok(None) => self.app_update.found(None),
                Err(message) => {
                    log_warn!("the RustBlox release list could not be read: {message}");
                    self.app_update.check_failed(message);
                }
            },
            Update::GameName { place_id, name } => {
                log_info!("place {place_id} is {name}");
                self.place_names.insert(place_id, name);
                self.sync_presence();
            }
            Update::AppDownload(AppDownload::Progress { done, total }) => {
                self.app_update.progress(done, total)
            }
            Update::AppDownload(AppDownload::Finished(outcome)) => match outcome {
                Ok(()) => {
                    let version = self
                        .app_update
                        .available
                        .as_ref()
                        .map(|release| release.version.clone())
                        .unwrap_or_default();
                    log_info!("RustBlox {version} is staged and takes effect on restart");
                    self.app_update.ready();
                    self.toasts.push(
                        ToastKind::Success,
                        format!("RustBlox {version} is ready"),
                        Some("Restart RustBlox to finish updating.".into()),
                    );
                }
                Err(message) => {
                    log_error!("the RustBlox update failed: {message}");
                    self.app_update.failed(message.clone());
                    self.toasts.error("The update failed", Some(message));
                }
            },
            Update::Swept(sweep) => {
                for problem in &sweep.problems {
                    log_warn!("cleanup: {problem}");
                }
                if !sweep.removed.is_empty() {
                    log_info!("cleanup: {}", sweep.summary());
                    self.toasts.push(
                        ToastKind::Success,
                        "Cleaned up old files",
                        Some(sweep.summary()),
                    );
                    self.rescan();
                }
                if !sweep.problems.is_empty() {
                    self.toasts.warning(
                        "Some files could not be removed",
                        Some(sweep.problems.join("; ")),
                    );
                }
            }
        }
    }

    fn finish_install(&mut self) {
        let in_flow = self.flow.stage == FlowStage::Preparing;

        match self.install.phase {
            InstallPhase::Succeeded => {
                if let Some(report) = self.install.report.clone() {
                    log_info!(
                        "installed Roblox {} into {}",
                        report.version,
                        report.directory.display()
                    );
                    if !report.unknown_packages.is_empty() {
                        log_warn!(
                            "Roblox published packages RustBlox does not know how to place: {}",
                            report.unknown_packages.join(", ")
                        );
                        self.toasts.warning(
                            "Some Roblox packages were skipped",
                            Some(format!(
                                "{} were not recognised, so this install may be incomplete. RustBlox needs updating to handle them.",
                                report.unknown_packages.join(", ")
                            )),
                        );
                    }

                    if !in_flow {
                        if report.already_present {
                            self.toasts.info("Roblox is already up to date");
                        } else {
                            self.toasts.push(
                                ToastKind::Success,
                                format!("Installed Roblox {}", report.version),
                                Some(format!(
                                    "Downloaded {}",
                                    crate::roblox::install::format_size(report.downloaded)
                                )),
                            );
                        }
                    }
                    self.latest_note = None;
                    self.tidy_after_install(&report.folder);
                }
                self.rescan();
                self.check_latest();
            }
            InstallPhase::Failed => {
                let message = self
                    .install
                    .failure
                    .as_ref()
                    .map(|failure| failure.message.clone())
                    .unwrap_or_default();
                log_error!("install failed: {message}");
            }
            InstallPhase::Cancelled => log_info!("install cancelled by the user"),
            _ => {}
        }

        if in_flow {
            self.advance_flow_after_install();
        }
    }

    pub fn can_install(&self) -> bool {
        !self.install.phase.is_busy() && !self.session.phase.is_busy()
    }

    pub fn install_roblox(&mut self, force: bool) {
        if !self.can_install() {
            return;
        }

        log_info!(
            "install requested on channel {}",
            self.settings.advanced.channel
        );

        let plan = InstallPlan {
            channel: self.settings.advanced.channel.clone(),
            versions_root: self.store.paths().versions_dir(),
            downloads_root: self.store.paths().downloads_dir(),
            force,
        };

        let cancel = self.install.begin();
        self.tasks.install(plan, cancel);
    }

    pub fn cancel_install(&mut self) {
        if self.install.phase.is_busy() {
            self.install.request_cancel();
        }
    }

    pub fn dismiss_install(&mut self) {
        self.install.reset();
        if !self.flow.stage.is_busy() {
            self.flow.reset();
        }
    }

    fn finish_launch(&mut self) {
        let target = self
            .session
            .target
            .as_ref()
            .map(LaunchTarget::headline)
            .unwrap_or_default();

        let record = match self.session.phase {
            Phase::Succeeded => {
                let report = self.session.report.clone();
                self.persisted.launch_count = self.persisted.launch_count.saturating_add(1);
                log_info!("launch succeeded: {target}");
                Some(LaunchRecord {
                    at: Local::now(),
                    outcome: LaunchOutcome::Succeeded,
                    target,
                    version: report.as_ref().and_then(|report| report.version.clone()),
                    detail: report.and_then(|report| report.note),
                })
            }
            Phase::Failed => {
                let failure = self.session.failure.clone();
                let message = failure
                    .as_ref()
                    .map(|failure| failure.message.clone())
                    .unwrap_or_default();
                log_error!("launch failed: {message}");
                if self.flow.stage != FlowStage::Launching {
                    self.toasts
                        .push(ToastKind::Error, "Launch failed", Some(message.clone()));
                }
                Some(LaunchRecord {
                    at: Local::now(),
                    outcome: LaunchOutcome::Failed,
                    target,
                    version: None,
                    detail: Some(message),
                })
            }
            Phase::Cancelled => {
                log_info!("launch cancelled by the user");
                Some(LaunchRecord {
                    at: Local::now(),
                    outcome: LaunchOutcome::Cancelled,
                    target,
                    version: None,
                    detail: None,
                })
            }
            _ => None,
        };

        if let Some(record) = record {
            self.persisted.last_launch = Some(record);
            self.state_dirty = true;
            self.flush_state();
        }

        self.last_poll = Instant::now() - BUSY_POLL;

        if self.flow.stage == FlowStage::Launching {
            self.finish_flow();
            return;
        }

        if self.session.phase == Phase::Succeeded && !self.stays_open_after_launch() {
            self.close_requested = true;
        }
    }

    pub fn start_launch_flow(&mut self, target: LaunchTarget) {
        if self.flow.stage.is_busy() || self.session.phase.is_busy() {
            return;
        }

        log_info!("launch flow requested: {}", target.headline());
        self.flow.begin(target.clone());

        let missing = self.detection.active().is_none();
        if self.settings.launch.update_roblox_on_launch || missing {
            self.install.reset();
            self.install_roblox(false);
            if !self.install.phase.is_busy() {
                self.flow.fail(
                    "Roblox could not be prepared.".into(),
                    Some("Another install or launch is still running.".into()),
                    false,
                );
            }
            return;
        }

        self.launch_in_flow();
    }

    fn launch_in_flow(&mut self) {
        let Some(target) = self.flow.target.clone() else {
            self.flow.reset();
            return;
        };

        if let Some(install) = self.detection.active() {
            let install_dir = install.version_dir.clone();
            if let Ok(exe) = std::env::current_exe() {
                let elevated_result = platform::spawn_elevated(
                    &exe,
                    &[
                        "--thewatcher".to_string(),
                        "0".to_string(),
                        install_dir.display().to_string(),
                    ],
                );

                if let Err(err) = elevated_result {
                    log_warn!("elevation cancelled or failed: {err}");
                    self.flow.fail(
                        "Administrator elevation was declined.".into(),
                        Some("TheWatcher Anti-Cheat requires administrator rights to run.".into()),
                        true,
                    );
                    return;
                }
            }
        }

        if self.settings.launch.multi_instance {
            platform::close_roblox_singleton_mutex();
        }

        self.flow.stage = FlowStage::Launching;
        self.session.reset();
        self.launch(target);
    }

    fn advance_flow_after_install(&mut self) {
        match self.install.phase {
            InstallPhase::Succeeded => {
                self.install.reset();
                self.launch_in_flow();
            }
            InstallPhase::Cancelled => {
                self.install.reset();
                self.flow
                    .fail("The launch was cancelled.".into(), None, true);
            }
            _ => {
                let failure = self.install.failure.clone();
                let message = failure
                    .as_ref()
                    .map(|failure| failure.message.clone())
                    .unwrap_or_else(|| "Roblox could not be prepared.".into());
                let hint = failure.as_ref().and_then(|failure| failure.hint.clone());
                self.install.reset();

                if self.detection.active().is_some() {
                    log_warn!("update check failed, starting the installed copy: {message}");
                    self.flow.note =
                        Some("The update check failed, starting the copy you have.".into());
                    self.launch_in_flow();
                } else {
                    self.flow.fail(message, hint, false);
                }
            }
        }
    }

    fn finish_flow(&mut self) {
        match self.session.phase {
            Phase::Succeeded => {
                if self.stays_open_after_launch() {
                    self.flow.stage = FlowStage::Watching;
                } else {
                    self.flow.stage = FlowStage::Finished;
                    self.close_requested = true;
                }
            }
            Phase::Cancelled => self
                .flow
                .fail("The launch was cancelled.".into(), None, true),
            _ => {
                let failure = self.session.failure.clone();
                self.flow.fail(
                    failure
                        .as_ref()
                        .map(|failure| failure.message.clone())
                        .unwrap_or_else(|| "Roblox could not be started.".into()),
                    failure.as_ref().and_then(|failure| failure.hint.clone()),
                    false,
                );
            }
        }
    }

    pub fn cancel_flow(&mut self) {
        match self.flow.stage {
            FlowStage::Preparing => self.cancel_install(),
            FlowStage::Launching => self.cancel_launch(),
            _ => {}
        }
    }

    pub fn dismiss_flow(&mut self) {
        self.flow.reset();
        self.install.reset();
        self.session.reset();
    }

    pub fn retry_flow(&mut self) {
        let Some(target) = self.flow.target.clone() else {
            return;
        };
        self.dismiss_flow();
        self.start_launch_flow(target);
    }

    pub fn flow_status(&self) -> FlowStatus {
        match self.flow.stage {
            FlowStage::Preparing => {
                let progress = self
                    .install
                    .progress
                    .as_ref()
                    .filter(|progress| progress.total > 0)
                    .map(|progress| progress.fraction());
                FlowStatus {
                    headline: self
                        .install
                        .active_stage()
                        .map(|row| row.stage.title().to_owned())
                        .unwrap_or_else(|| "Checking for Roblox updates".into()),
                    detail: self.install.subline(),
                    progress,
                }
            }
            FlowStage::Launching => FlowStatus {
                headline: self
                    .session
                    .active_step()
                    .map(|step| step.id.title().to_owned())
                    .unwrap_or_else(|| "Starting Roblox".into()),
                detail: self
                    .flow
                    .note
                    .clone()
                    .unwrap_or_else(|| self.session.subline()),
                progress: None,
            },
            FlowStage::Finished => FlowStatus {
                headline: "Roblox is running".into(),
                detail: "Have fun.".into(),
                progress: Some(1.0),
            },
            FlowStage::Watching => FlowStatus {
                headline: "Roblox is running".into(),
                detail: match self.presence_status() {
                    super::presence::Status::On => {
                        format!("{}, and Discord is being told.", self.activity.summary())
                    }
                    _ if self.security.is_running() => "TheWatcher is active.".into(),
                    other => other.label(),
                },
                progress: Some(1.0),
            },
            FlowStage::Failed => FlowStatus {
                headline: if self.flow.cancelled {
                    "Launch cancelled".into()
                } else {
                    "Roblox could not be started".into()
                },
                detail: self.flow.failure.clone().unwrap_or_default(),
                progress: None,
            },
            FlowStage::Idle => FlowStatus {
                headline: "Ready".into(),
                detail: String::new(),
                progress: None,
            },
        }
    }

    pub fn scan_options(&self) -> ScanOptions {
        ScanOptions {
            managed_root: Some(self.store.paths().data_dir().to_path_buf()),
            custom_root: self.settings.advanced.custom_install_root.clone(),
            pinned: self.settings.advanced.pinned_version_folder.clone(),
        }
    }

    pub fn rescan(&mut self) {
        let options = self.scan_options();
        self.tasks.scan(options);
    }

    pub fn check_latest(&mut self) {
        self.tasks
            .check_latest(self.settings.advanced.channel.clone());
    }

    pub fn check_app_update(&mut self) {
        if self.tasks.is_app_busy() {
            return;
        }
        self.app_update.begin_check();
        self.tasks.check_app_update();
    }

    pub fn start_app_update(&mut self) {
        if self.tasks.is_app_busy() {
            return;
        }
        let Some(release) = self.app_update.available.clone() else {
            return;
        };
        let Some(exe) = self.exe_path.clone() else {
            self.toasts.error(
                "The RustBlox executable could not be located",
                Some("Updating needs a real path on disk.".into()),
            );
            return;
        };

        log_info!(
            "downloading RustBlox {} from {}",
            release.version,
            release.url
        );
        self.app_update.begin_download(release.size);
        self.tasks.download_app_update(release, exe);
    }

    pub fn restart_for_update(&mut self) {
        let Some(exe) = self.exe_path.clone() else {
            return;
        };
        match platform::spawn_detached(&exe, &[], exe.parent()) {
            Ok(_) => {
                log_info!("restarting into the new build");
                self.close_requested = true;
            }
            Err(err) => {
                log_error!("the new build could not be started: {err}");
                self.toasts.error(
                    "RustBlox could not restart itself",
                    Some(format!("{err} Close and open RustBlox yourself to finish.")),
                );
            }
        }
    }

    pub fn managed_folders(&self) -> Vec<String> {
        versions::managed_folders(
            &self.detection.installations,
            &self.store.paths().versions_dir(),
        )
    }

    pub fn update_available(&self) -> Option<&Deployment> {
        let latest = self.latest.as_ref()?;
        if versions::update_available(&self.managed_folders(), &latest.folder) {
            Some(latest)
        } else {
            None
        }
    }

    fn tidy_after_install(&mut self, folder: &str) {
        if self.roblox.player_running() {
            log_info!("cleanup skipped while the Roblox client is running");
            return;
        }

        let mut keep_versions = vec![folder.to_owned()];
        if let Some(pinned) = self.settings.advanced.pinned_version_folder.clone() {
            keep_versions.push(pinned);
        }
        let keep_downloads = if self.settings.advanced.keep_downloads {
            vec![folder.to_owned()]
        } else {
            Vec::new()
        };

        self.tasks.sweep(
            self.store.paths().versions_dir(),
            self.store.paths().downloads_dir(),
            keep_versions,
            keep_downloads,
        );
    }

    pub fn default_target(&self) -> LaunchTarget {
        match self.settings.launch.startup_target {
            StartupTarget::App => LaunchTarget::App,
            StartupTarget::LastPlayed => self
                .persisted
                .last_quick_target
                .and_then(|place_id| {
                    self.settings
                        .launch
                        .quick_targets
                        .iter()
                        .find(|entry| entry.place_id == place_id)
                        .map(|entry| LaunchTarget::Place {
                            place_id,
                            label: Some(entry.name.clone()),
                        })
                })
                .unwrap_or(LaunchTarget::App),
        }
    }

    pub fn can_launch(&self) -> bool {
        !self.session.phase.is_busy()
    }

    pub fn launch(&mut self, target: LaunchTarget) {
        if !self.can_launch() {
            return;
        }

        if let LaunchTarget::Place { place_id, .. } = &target {
            self.persisted.last_quick_target = Some(*place_id);
            self.state_dirty = true;
        }

        log_info!("launch requested: {}", target.headline());

        let plan = LaunchPlan {
            target: target.clone(),
            scan: self.scan_options(),
            verify: self.settings.advanced.verify_before_launch,
            flag_profile: if self.settings.advanced.apply_flag_profile {
                Some(self.flags.clone())
            } else {
                None
            },
            game: self.game_plan(),
            mods: Some(self.mod_plan()),
            backup_dir: self.store.paths().backup_dir(),
            extra_arguments: self.settings.advanced.extra_player_arguments.clone(),
            timeout: Duration::from_secs(self.settings.launch.launch_timeout_secs),
            allow_when_running: !self.settings.launch.warn_when_already_running,
        };

        let cancel = self.session.begin(target);
        self.tasks.launch(plan, cancel);
    }

    pub fn cancel_launch(&mut self) {
        if self.session.phase.is_busy() {
            self.session.request_cancel();
        }
    }

    pub fn dismiss_launch(&mut self) {
        self.session.reset();
        if !self.flow.stage.is_busy() {
            self.flow.reset();
        }
    }

    pub fn mark_settings_dirty(&mut self) {
        self.settings_dirty_at = Some(Instant::now());
    }

    pub fn settings_pending(&self) -> bool {
        self.settings_dirty_at.is_some()
    }

    pub fn flush_settings(&mut self) {
        if self.settings_dirty_at.take().is_none() {
            return;
        }
        let notes = self.settings.validate();
        for note in notes {
            log_warn!("settings adjusted: {note}");
        }
        if let Err(err) = self.store.save_settings(&self.settings) {
            log_error!("settings could not be saved: {err}");
            self.toasts
                .error("Settings could not be saved", Some(err.to_string()));
        }
    }

    pub fn mark_state_dirty(&mut self) {
        self.state_dirty = true;
    }

    pub fn flush_state(&mut self) {
        if !self.state_dirty {
            return;
        }
        self.state_dirty = false;
        if let Err(err) = self.store.save_state(&self.persisted) {
            log_error!("state could not be saved: {err}");
        }
    }

    pub fn game_plan(&self) -> Option<GamePlan> {
        if !self.settings.game.manage {
            return None;
        }
        let path = gamesettings::settings_file()?;
        Some(GamePlan {
            path,
            changes: self.settings.game.changes(),
            lock: self.settings.game.lock,
        })
    }

    fn refresh_activity(&mut self) {
        if !self.settings.launch.track_activity || !self.roblox.player_running() {
            if self.activity != Activity::default() {
                self.activity = Activity::default();
                self.activity_since = None;
            }
            self.sync_presence();
            return;
        }

        if self
            .activity_checked_at
            .is_some_and(|at| at.elapsed() < ACTIVITY_PROBE)
        {
            return;
        }
        self.activity_checked_at = Some(Instant::now());

        let found = crate::roblox::log_dir()
            .map(|dir| activity::read(&dir))
            .unwrap_or_default();

        if !found.is_same_place(&self.activity) {
            log_info!("activity: {}", found.summary());
            self.activity_since = found.in_game.then(crate::discord::now);
            if let Some(place) = found.place_id.filter(|_| found.in_game) {
                self.look_up_place(place, found.universe_id);
            }
        }
        self.activity = found;
        self.sync_presence();
    }

    fn look_up_place(&mut self, place_id: u64, universe_id: Option<u64>) {
        if !self.settings.discord.is_usable()
            || !self.settings.discord.show_place_name
            || self.place_names.contains_key(&place_id)
        {
            return;
        }
        self.tasks.look_up_place(place_id, universe_id);
    }

    pub fn presence_status(&self) -> super::presence::Status {
        self.presence.status()
    }

    fn sync_presence(&mut self) {
        let wanted = self.settings.discord.is_usable();
        if wanted != self.presence.is_running() {
            if wanted {
                log_info!("turning the Discord presence on");
                self.presence
                    .start(self.settings.discord.application_id.clone());
            } else {
                self.presence.stop();
            }
        }
        if !wanted {
            return;
        }

        if !self.roblox.player_running() {
            self.presence.hide();
            return;
        }

        let line = if self.settings.discord.streamer_mode {
            "Playing Roblox".to_owned()
        } else {
            match (self.activity.in_game, self.activity.place_id) {
                (true, Some(place)) => match self.place_names.get(&place) {
                    Some(name) => format!("Playing {name}"),
                    None => "In a game".to_owned(),
                },
                (true, None) => "In a game".to_owned(),
                (false, _) => "In the Roblox app".to_owned(),
            }
        };

        let note = if self.settings.discord.streamer_mode {
            String::new()
        } else {
            match (self.activity.in_game, self.activity.place_id) {
                (true, Some(place)) if self.settings.discord.show_place_name => {
                    format!("Place {place}")
                }
                _ => String::new(),
            }
        };

        self.presence.show(crate::discord::Details {
            line,
            note,
            started_at: self.activity_since,
        });
    }

    pub fn refresh_presence(&mut self) {
        self.presence.stop();
        self.sync_presence();
    }

    pub fn stays_open_after_launch(&self) -> bool {
        self.settings.discord.is_usable() || !self.settings.launch.close_after_launch
    }

    pub fn left_the_client(&self) -> bool {
        self.client_seen && !self.roblox.player_running()
    }

    pub fn refresh_shortcuts(&mut self, force: bool) {
        if !force
            && self
                .shortcuts_checked_at
                .is_some_and(|at| at.elapsed() < DENIED_PROBE)
        {
            return;
        }
        self.shortcuts_checked_at = Some(Instant::now());
        if let Some(exe) = self.exe_path.clone() {
            if self.settings.shortcuts.start_menu && (force || !shortcuts::Kind::StartMenu.exists())
            {
                let _ = shortcuts::create(shortcuts::Kind::StartMenu, &exe);
            }
            if self.settings.shortcuts.desktop && (force || !shortcuts::Kind::Desktop.exists()) {
                let _ = shortcuts::create(shortcuts::Kind::Desktop, &exe);
            }
        }
        self.shortcuts = shortcuts::Present::read();
    }

    pub fn toggle_shortcut(&mut self, kind: shortcuts::Kind) {
        let will_exist = !kind.exists();
        match kind {
            shortcuts::Kind::StartMenu => {
                self.settings.shortcuts.start_menu = will_exist;
                self.mark_settings_dirty();
                self.flush_settings();
            }
            shortcuts::Kind::Desktop => {
                self.settings.shortcuts.desktop = will_exist;
                self.mark_settings_dirty();
                self.flush_settings();
            }
            _ => {}
        }

        let outcome = if !will_exist {
            shortcuts::remove(kind).map(|()| None)
        } else {
            match self.exe_path.clone() {
                Some(exe) => shortcuts::create(kind, &exe).map(Some),
                None => Err(crate::error::Error::invalid(
                    "RustBlox cannot find its own executable to point at",
                )),
            }
        };

        match outcome {
            Ok(Some(path)) => {
                log_info!("wrote the shortcut {}", path.display());
                self.toasts.success(format!("{} added", kind.label()));
            }
            Ok(None) => {
                log_info!("removed the {} shortcut", kind.label());
                self.toasts.success(format!("{} removed", kind.label()));
            }
            Err(err) => {
                log_error!("the shortcut could not be written: {err}");
                self.toasts
                    .error("That shortcut could not be written", Some(err.to_string()));
            }
        }

        self.refresh_shortcuts(true);
    }

    pub fn clean_roblox_cache(&mut self) -> u64 {
        let mut freed = 0_u64;
        if let Some(roblox) = crate::roblox::local_dir() {
            for sub in ["logs", "downloads", "crashes", "LocalStorage", "http"] {
                let target = roblox.join(sub);
                if target.is_dir() {
                    freed += remove_folder_contents(&target);
                }
            }
        }
        let roblox_temp = std::env::temp_dir().join("Roblox");
        if roblox_temp.is_dir() {
            freed += remove_folder_contents(&roblox_temp);
        }
        let downloads = self.store.paths().downloads_dir();
        if downloads.is_dir() {
            freed += remove_folder_contents(&downloads);
        }
        freed
    }

    pub fn roblox_cache_size(&self) -> u64 {
        let mut total = 0_u64;
        if let Some(roblox) = crate::roblox::local_dir() {
            for sub in ["logs", "downloads", "crashes", "LocalStorage", "http"] {
                let target = roblox.join(sub);
                if target.is_dir() {
                    total += folder_size(&target);
                }
            }
        }
        let roblox_temp = std::env::temp_dir().join("Roblox");
        if roblox_temp.is_dir() {
            total += folder_size(&roblox_temp);
        }
        let downloads = self.store.paths().downloads_dir();
        if downloads.is_dir() {
            total += folder_size(&downloads);
        }
        total
    }

    pub fn check_auto_clean_cache(&mut self) {
        if !self.settings.advanced.auto_clean_cache {
            return;
        }
        let threshold_bytes = self
            .settings
            .advanced
            .auto_clean_threshold_mb
            .saturating_mul(1024 * 1024);
        let size = self.roblox_cache_size();
        if size >= threshold_bytes {
            let freed = self.clean_roblox_cache();
            if freed > 0 {
                log_info!(
                    "auto cleaned cache, freed {}",
                    crate::roblox::install::format_size(freed)
                );
                self.toasts.info(format!(
                    "Auto-cleaned {} of Roblox cache",
                    crate::roblox::install::format_size(freed)
                ));
            }
        }
    }

    pub fn mod_plan(&self) -> ModPlan {
        ModPlan {
            root: self.store.paths().mods_dir(),
            originals: self.store.paths().mod_originals_dir(),
            enabled: self.settings.mods.enabled,
        }
    }

    pub fn refresh_mods(&mut self, force: bool) {
        if !force
            && self
                .mods_checked_at
                .is_some_and(|at| at.elapsed() < DENIED_PROBE)
        {
            return;
        }
        self.mods_checked_at = Some(Instant::now());
        self.mods = mods::scan(&self.store.paths().mods_dir());
    }

    pub fn apply_mods_now(&mut self) {
        let plan = self.mod_plan();
        let Some(install) = self.detection.active().cloned() else {
            self.toasts
                .error("No Roblox to change", Some("Install it first.".into()));
            return;
        };

        if let Err(err) = crate::util::fs::ensure_dir(&plan.root) {
            log_error!("the mods folder could not be created: {err}");
            self.toasts.error(
                "The mods folder could not be created",
                Some(err.to_string()),
            );
            return;
        }

        let originals = plan.originals.join(&install.folder_id);
        mods::forget_other_versions(&plan.originals, &install.folder_id);

        let outcome = if plan.enabled {
            mods::apply(&install.version_dir, &plan.root, &originals)
        } else {
            mods::restore_all(&install.version_dir, &originals).map(|restored| mods::Report {
                restored,
                ..mods::Report::default()
            })
        };

        match outcome {
            Ok(report) => {
                log_info!("mods: {}", report.summary());
                for refused in &report.refused {
                    log_warn!("mod file refused: {refused}");
                }
                self.toasts.success(capitalise(&report.summary()));
            }
            Err(err) => {
                log_error!("mods could not be applied: {err}");
                self.toasts
                    .error("Mods could not be applied", Some(err.to_string()));
            }
        }

        self.refresh_mods(true);
    }

    pub fn choose_font(&mut self) {
        let Some(install) = self.detection.active().cloned() else {
            self.toasts
                .error("No Roblox to read from", Some("Install it first.".into()));
            return;
        };
        let Some(source) = rfd::FileDialog::new()
            .set_title("Choose a font for Roblox")
            .add_filter("Fonts", &["ttf", "otf"])
            .pick_file()
        else {
            return;
        };

        match mods::install_font(&self.store.paths().mods_dir(), &install, &source) {
            Ok(count) => {
                log_info!("rewrote {count} font families to use {}", source.display());
                self.toasts
                    .success(format!("{count} font families now use that font"));
                self.settings.mods.enabled = true;
                self.mark_settings_dirty();
                self.flush_settings();
                self.apply_mods_now();
            }
            Err(err) => {
                log_error!("the font could not be installed: {err}");
                self.toasts
                    .error("That font could not be used", Some(err.to_string()));
                self.refresh_mods(true);
            }
        }
    }

    pub fn clear_font(&mut self) {
        match mods::remove_font(&self.store.paths().mods_dir()) {
            Ok(()) => {
                self.toasts.success("Roblox goes back to its own fonts");
                self.apply_mods_now();
            }
            Err(err) => {
                log_error!("the font could not be removed: {err}");
                self.toasts
                    .error("The font could not be removed", Some(err.to_string()));
            }
        }
    }

    pub fn choose_death_sound(&mut self) {
        let Some(source) = rfd::FileDialog::new()
            .set_title("Choose a death sound for Roblox")
            .add_filter("Audio", &["ogg", "wav", "mp3"])
            .pick_file()
        else {
            return;
        };

        match mods::install_custom_death_sound(&self.store.paths().mods_dir(), &source) {
            Ok(()) => {
                log_info!("installed custom death sound from {}", source.display());
                self.toasts.success("Custom death sound installed");
                self.settings.mods.enabled = true;
                self.settings.mods.death_sound = crate::config::DeathSoundPreset::Custom;
                self.mark_settings_dirty();
                self.flush_settings();
                self.refresh_mods(true);
                self.apply_mods_now();
            }
            Err(err) => {
                log_error!("custom death sound could not be installed: {err}");
                self.toasts
                    .error("That sound could not be installed", Some(err.to_string()));
            }
        }
    }

    pub fn choose_custom_cursor(&mut self) {
        let Some(source) = rfd::FileDialog::new()
            .set_title("Choose a custom mouse cursor image")
            .add_filter("Images", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };

        match crate::roblox::cursor::install_custom_cursor(&self.store.paths().mods_dir(), &source)
        {
            Ok(()) => {
                log_info!("installed custom cursor from {}", source.display());
                self.toasts.success("Custom cursor installed");
                self.settings.mods.enabled = true;
                self.settings.mods.cursor = crate::config::CursorPreset::Custom;
                self.mark_settings_dirty();
                self.flush_settings();
                self.refresh_mods(true);
                self.apply_mods_now();
            }
            Err(err) => {
                log_error!("custom cursor could not be installed: {err}");
                self.toasts
                    .error("That cursor could not be installed", Some(err.to_string()));
            }
        }
    }

    pub fn add_account(&mut self, cookie: &str) {
        match crate::roblox::account::fetch_account_details(cookie) {
            Ok((id, username, display_name)) => {
                let clean_cookie = crate::roblox::account::sanitize_cookie(cookie);
                self.accounts.retain(|acc| acc.id != id);
                let profile = crate::roblox::account::AccountProfile {
                    id,
                    username: username.clone(),
                    display_name: display_name.clone(),
                    cookie: clean_cookie.clone(),
                    created_at: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
                };
                self.accounts.push(profile);
                let _ = crate::roblox::account::save_accounts(
                    &self.store.paths().accounts_file(),
                    &self.accounts,
                );
                let _ = crate::roblox::account::apply_account_session(&clean_cookie);
                self.active_account_id = Some(id);
                self.toasts
                    .success(format!("Logged in as @{username} ({display_name})"));
            }
            Err(err) => {
                self.toasts
                    .error("Could not verify Roblox account", Some(err.to_string()));
            }
        }
    }

    pub fn switch_account(&mut self, id: u64) {
        if let Some(acc) = self.accounts.iter().find(|acc| acc.id == id).cloned() {
            let _ = crate::roblox::account::apply_account_session(&acc.cookie);
            self.active_account_id = Some(id);
            self.toasts.info(format!(
                "Switched to @{} ({})",
                acc.username, acc.display_name
            ));
        }
    }

    pub fn remove_account(&mut self, id: u64) {
        self.accounts.retain(|acc| acc.id != id);
        if self.active_account_id == Some(id) {
            self.active_account_id = self.accounts.first().map(|acc| acc.id);
            if let Some(active_id) = self.active_account_id {
                self.switch_account(active_id);
            }
        }
        let _ = crate::roblox::account::save_accounts(
            &self.store.paths().accounts_file(),
            &self.accounts,
        );
        self.toasts.info("Account removed");
    }

    pub fn unlock_game_settings(&mut self) {
        let Some(path) = gamesettings::settings_file() else {
            return;
        };
        match gamesettings::unlock(&path) {
            Ok(true) => {
                self.settings.game.lock = false;
                self.mark_settings_dirty();
                self.flush_settings();
                self.toasts.success("Roblox can write its settings again");
            }
            Ok(false) => {}
            Err(err) => {
                log_error!("the settings file could not be unlocked: {err}");
                self.toasts
                    .error("The file could not be unlocked", Some(err.to_string()));
            }
        }
        self.refresh_game_snapshot(true);
    }

    pub fn refresh_game_snapshot(&mut self, force: bool) {
        if !force
            && self
                .game_checked_at
                .is_some_and(|at| at.elapsed() < DENIED_PROBE)
        {
            return;
        }
        self.game_checked_at = Some(Instant::now());
        self.game = Snapshot::read();
    }

    pub fn mark_game_dirty(&mut self) {
        self.mark_settings_dirty();
        self.game_dirty_at = Some(Instant::now());
    }

    pub fn commit_game_settings(&mut self) {
        self.mark_game_dirty();
        self.flush_settings();
        self.flush_game_settings();
    }

    pub fn flush_game_settings(&mut self) {
        if self.game_dirty_at.take().is_none() {
            return;
        }

        let Some(path) = gamesettings::settings_file() else {
            return;
        };

        if !self.settings.game.manage {
            match gamesettings::unlock(&path) {
                Ok(true) => log_info!("released the lock on {}", path.display()),
                Ok(false) => {}
                Err(err) => log_warn!("the lock could not be released: {err}"),
            }
            self.refresh_game_snapshot(true);
            return;
        }

        let changes = self.settings.game.changes();
        let lock = self.settings.game.lock;
        match gamesettings::apply(&path, &changes, lock, &self.store.paths().backup_dir()) {
            Ok(report) => {
                if report.outcome == gamesettings::Outcome::Written {
                    log_info!("{} in {}", report.summary(), report.path.display());
                }
            }
            Err(err) => {
                log_error!("game settings could not be written: {err}");
                self.toasts
                    .error("Game settings could not be written", Some(err.to_string()));
            }
        }
        self.refresh_game_snapshot(true);
    }

    pub fn refresh_denied_flags(&mut self) {
        if self
            .denied_checked_at
            .is_some_and(|at| at.elapsed() < DENIED_PROBE)
        {
            return;
        }
        self.denied_checked_at = Some(Instant::now());
        self.denied_flags = flags::client_log_dir()
            .map(|dir| flags::denied_by_client(&dir))
            .unwrap_or_default();
    }

    pub fn denied_active_flags(&self) -> Vec<String> {
        self.flags
            .active()
            .map(|entry| entry.key.clone())
            .filter(|key| {
                self.denied_flags
                    .iter()
                    .any(|denied| denied.eq_ignore_ascii_case(key))
            })
            .collect()
    }

    pub fn clear_refused_flags(&mut self) {
        let refused = self.denied_active_flags();
        if refused.is_empty() {
            return;
        }
        self.flags.entries.retain(|entry| {
            !refused
                .iter()
                .any(|key| key.eq_ignore_ascii_case(&entry.key))
        });
        self.denied_flags
            .retain(|key| !refused.iter().any(|r| r.eq_ignore_ascii_case(key)));
        self.commit_flags();
        self.toasts
            .success("Removed refused flags from active profile");
    }

    pub fn mark_flags_dirty(&mut self) {
        self.flags_dirty_at = Some(Instant::now());
    }

    pub fn commit_flags(&mut self) {
        self.mark_flags_dirty();
        self.flush_flags();
    }

    pub fn flush_flags(&mut self) {
        if self.flags_dirty_at.take().is_none() {
            return;
        }

        self.flags.sort();
        let profile_name = self.settings.advanced.active_flag_profile.clone();
        if let Err(err) = flags::save_named_profile(
            &self.store.paths().flag_profiles_dir(),
            &profile_name,
            &self.flags,
        ) {
            log_error!("flag profile could not be saved: {err}");
            self.toasts
                .error("Flag profile could not be saved", Some(err.to_string()));
            return;
        }

        self.write_flags_to_client(false);
    }

    pub fn flag_profiles(&self) -> Vec<String> {
        flags::list_profiles(&self.store.paths().flag_profiles_dir())
    }

    pub fn switch_flag_profile(&mut self, name: &str) {
        let clean = flags::sanitize_profile_name(name);
        self.flush_flags();
        match flags::load_named_profile(&self.store.paths().flag_profiles_dir(), &clean) {
            Ok(profile) => {
                self.flags = profile;
                self.settings.advanced.active_flag_profile = clean.clone();
                self.mark_settings_dirty();
                self.flush_settings();
                self.write_flags_to_client(false);
                self.toasts.info(format!("Switched to profile \"{clean}\""));
            }
            Err(err) => {
                self.toasts
                    .error("Could not switch profile", Some(err.to_string()));
            }
        }
    }

    pub fn create_flag_profile(&mut self, name: &str, clone_current: bool) {
        let clean = flags::sanitize_profile_name(name);
        let new_profile = if clone_current {
            self.flags.clone()
        } else {
            FlagProfile::default()
        };
        if let Err(err) = flags::save_named_profile(
            &self.store.paths().flag_profiles_dir(),
            &clean,
            &new_profile,
        ) {
            self.toasts
                .error("Could not create profile", Some(err.to_string()));
            return;
        }
        self.switch_flag_profile(&clean);
    }

    pub fn delete_flag_profile(&mut self, name: &str) {
        let clean = flags::sanitize_profile_name(name);
        if clean == "default" {
            self.toasts
                .warning("The default profile cannot be deleted", None);
            return;
        }
        let _ = flags::delete_named_profile(&self.store.paths().flag_profiles_dir(), &clean);
        if self.settings.advanced.active_flag_profile == clean {
            self.switch_flag_profile("default");
        } else {
            self.toasts.info(format!("Deleted profile \"{clean}\""));
        }
    }

    pub fn apply_preset_flags(&mut self, preset_index: usize) {
        let Some(preset) = flags::PRESETS.get(preset_index) else {
            return;
        };
        for &(key, val) in preset.flags {
            self.flags
                .set(key.to_string(), flags::FlagValue::from_input(val));
        }
        self.commit_flags();
        self.toasts
            .success(format!("Applied \"{}\" preset", preset.name));
    }

    pub fn write_flags_now(&mut self) {
        self.mark_flags_dirty();
        self.flush_flags();
        self.write_flags_to_client(true);
    }

    fn write_flags_to_client(&mut self, forced: bool) {
        if !forced && !self.settings.advanced.apply_flag_profile {
            return;
        }
        let Some(install) = self.detection.active().cloned() else {
            if forced {
                self.toasts
                    .error("No Roblox to write to", Some("Install it first.".into()));
            }
            return;
        };

        match flags::apply_to(&install, &self.flags, &self.store.paths().backup_dir()) {
            Ok(report) if report.unchanged => {
                if forced {
                    self.toasts.success("The client file already matches");
                }
            }
            Ok(report) => {
                log_info!(
                    "wrote {} flags to {}",
                    report.count,
                    report.written.display()
                );
                if forced {
                    self.toasts
                        .success(format!("Wrote {} flags to the client", report.count));
                }
            }
            Err(err) => {
                log_error!("flags could not be applied: {err}");
                self.toasts
                    .error("Flags could not be applied", Some(err.to_string()));
            }
        }
    }

    pub fn client_flag_file(&self) -> Option<PathBuf> {
        self.detection
            .active()
            .map(|install| install.client_settings_file())
    }

    pub fn reset_flags(&mut self) {
        self.flags = FlagProfile::default();
        self.commit_flags();
        self.toasts.success("Flags reset");
    }

    pub fn refresh_protocol(&mut self) {
        self.protocol = platform::protocol::inspect(PLAYER_SCHEME).ok();
        self.deeplink = platform::protocol::inspect(DEEPLINK_SCHEME).ok();
    }

    pub fn register_protocol(&mut self, scheme: &str) {
        let Some(exe) = self.exe_path.clone() else {
            self.toasts.error(
                "The RustBlox executable could not be located",
                Some("Registration needs a real path on disk.".into()),
            );
            return;
        };

        match platform::protocol::register(scheme, &exe) {
            Ok(()) => {
                log_info!("registered {scheme} to {}", exe.display());
                self.toasts.push(
                    ToastKind::Success,
                    format!("RustBlox now handles {scheme} links"),
                    Some("The previous handler was saved and can be restored.".into()),
                );
            }
            Err(err) => {
                log_error!("could not register {scheme}: {err}");
                self.toasts
                    .error("Registration failed", Some(err.to_string()));
            }
        }
        self.refresh_protocol();
    }

    pub fn restore_protocol(&mut self, scheme: &str) {
        match platform::protocol::restore(scheme) {
            Ok(()) => {
                log_info!("restored the previous handler for {scheme}");
                self.toasts
                    .success(format!("Restored the previous {scheme} handler"));
            }
            Err(err) => {
                log_error!("could not restore {scheme}: {err}");
                self.toasts.error("Restore failed", Some(err.to_string()));
            }
        }
        self.refresh_protocol();
    }

    pub fn uninstall(&mut self, remove_settings: bool) {
        if self.roblox.player_running() {
            self.toasts.warning(
                "Close Roblox first",
                Some("RustBlox cannot remove files the client is using.".into()),
            );
            return;
        }

        let plan = crate::uninstall::Plan {
            remove_settings,
            remove_executable: true,
        };

        log_info!("uninstalling, settings removed: {remove_settings}");
        self.settings_dirty_at = None;
        self.state_dirty = false;

        let report = crate::uninstall::run(self.store.paths(), plan, self.exe_path.as_deref());
        for problem in &report.problems {
            log_warn!("uninstall: {problem}");
        }

        if report.problems.is_empty() {
            log_info!("uninstall finished: {}", report.summary());
            self.close_requested = true;
            return;
        }

        self.toasts.error(
            "Some files could not be removed",
            Some(report.problems.join("; ")),
        );
    }

    pub fn open_path(&mut self, path: PathBuf) {
        if let Err(err) = platform::open_path(&path) {
            self.toasts
                .error("Could not open that location", Some(err.to_string()));
        }
    }

    pub fn open_url(&mut self, url: &str) {
        if let Err(err) = platform::open_url(url) {
            self.toasts
                .error("Could not open that link", Some(err.to_string()));
        }
    }

    pub fn pick_install_folder(&mut self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_title("Select a Roblox folder")
            .pick_folder()
    }

    pub fn shutdown(&mut self) -> Result<()> {
        self.settings_dirty_at = Some(Instant::now());
        self.flush_settings();
        self.state_dirty = true;
        self.flush_state();
        self.mark_flags_dirty();
        self.flush_flags();
        if self.settings.game.manage {
            self.game_dirty_at = Some(Instant::now());
            self.flush_game_settings();
        }
        self.security.stop();
        self.presence.stop();
        Ok(())
    }
}

fn capitalise(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

fn remove_folder_contents(dir: &std::path::Path) -> u64 {
    let mut bytes = 0_u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Ok(meta) = entry.metadata() {
                bytes += meta.len();
            }
            let _ = std::fs::remove_file(&path);
        } else if path.is_dir() {
            bytes += remove_folder_contents(&path);
            let _ = std::fs::remove_dir(&path);
        }
    }
    bytes
}

fn folder_size(dir: &std::path::Path) -> u64 {
    let mut bytes = 0_u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Ok(meta) = entry.metadata() {
                bytes += meta.len();
            }
        } else if path.is_dir() {
            bytes += folder_size(&path);
        }
    }
    bytes
}
