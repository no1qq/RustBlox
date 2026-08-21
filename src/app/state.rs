use std::path::PathBuf;
use std::time::{Duration, Instant};

use chrono::Local;

use crate::cli::Invocation;
use crate::config::{LaunchOutcome, LaunchRecord, Settings, StartupTarget, State, Store};
use crate::error::Result;
use crate::platform::{self, SchemeRegistration};
use crate::roblox::deploy::Deployment;
use crate::roblox::detect::{Detection, ScanOptions};
use crate::roblox::flags::{self, FlagProfile};
use crate::roblox::installer::InstallPlan;
use crate::roblox::launch::{LaunchPlan, LaunchTarget};
use crate::roblox::process::RobloxStatus;
use crate::roblox::versions;
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

pub struct AppState {
    pub store: Store,
    pub settings: Settings,
    pub persisted: State,
    pub detection: Detection,
    pub latest: Option<Deployment>,
    pub latest_note: Option<String>,
    pub roblox: RobloxStatus,
    pub flags: FlagProfile,
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

    settings_dirty_at: Option<Instant>,
    flags_dirty_at: Option<Instant>,
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

        let flags = match flags::load_profile(&store.paths().flag_profiles_dir()) {
            Ok(profile) => profile,
            Err(err) => {
                log_error!("flag profile could not be read: {err}");
                startup_notes.push(format!("flag profile could not be read: {err}"));
                FlagProfile::default()
            }
        };

        let mut app = Self {
            store,
            settings,
            persisted: loaded_state.value,
            detection: Detection::default(),
            latest: None,
            latest_note: None,
            roblox: RobloxStatus::default(),
            flags,
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
            settings_dirty_at: None,
            flags_dirty_at: None,
            state_dirty: false,
            last_poll: Instant::now() - IDLE_POLL,
            last_theme_probe: Instant::now() - THEME_PROBE,
            initial_scan_done: false,
        };

        app.refresh_protocol();
        if let Some(exe) = app.exe_path.clone() {
            crate::selfupdate::clear_retired(&exe);
        }
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

        if self.last_theme_probe.elapsed() >= THEME_PROBE {
            self.last_theme_probe = Instant::now();
            if let Some(dark) = platform::system_dark_mode() {
                self.system_dark = dark;
            }
        }

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
                if was_running && !self.roblox.player_running() {
                    log_info!("the Roblox client closed");
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
                    self.app_update.found(Some(release));
                }
                Ok(None) => self.app_update.found(None),
                Err(message) => {
                    log_warn!("the RustBlox release list could not be read: {message}");
                    self.app_update.check_failed(message);
                }
            },
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

        if self.session.phase == Phase::Succeeded {
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
            Phase::Succeeded => self.flow.stage = FlowStage::Finished,
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
        if let Err(err) = flags::save_profile(&self.store.paths().flag_profiles_dir(), &self.flags)
        {
            log_error!("flag profile could not be saved: {err}");
            self.toasts
                .error("Flag profile could not be saved", Some(err.to_string()));
            return;
        }

        self.write_flags_to_client();
    }

    fn write_flags_to_client(&mut self) {
        if !self.settings.advanced.apply_flag_profile {
            return;
        }
        let Some(install) = self.detection.active().cloned() else {
            return;
        };

        match flags::apply_to(&install, &self.flags, &self.store.paths().backup_dir()) {
            Ok(report) if report.unchanged => {}
            Ok(report) => log_info!(
                "wrote {} flags to {}",
                report.count,
                report.written.display()
            ),
            Err(err) => {
                log_error!("flags could not be applied: {err}");
                self.toasts
                    .error("Flags could not be applied", Some(err.to_string()));
            }
        }
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
        Ok(())
    }
}
