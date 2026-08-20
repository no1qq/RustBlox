use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;

use std::path::PathBuf;

use crate::roblox::deploy::{self, Deployment};
use crate::roblox::detect::Detection;
use crate::roblox::installer::{InstallEvent, InstallPlan, Installer};
use crate::roblox::launch::{LaunchEvent, LaunchPlan, Launcher};
use crate::roblox::process::RobloxStatus;
use crate::roblox::versions::{self, Sweep};
use crate::roblox::{detect, process};

#[derive(Debug)]
pub enum Update {
    Scanned(Box<Detection>),
    Processes(RobloxStatus),
    Launch(LaunchEvent),
    Install(InstallEvent),
    Latest(Box<std::result::Result<Deployment, String>>),
    Swept(Sweep),
}

pub struct Tasks {
    sender: Sender<Update>,
    receiver: Receiver<Update>,
    repaint: Option<egui::Context>,
    scanning: bool,
    polling: bool,
    checking: bool,
    sweeping: bool,
}

impl Default for Tasks {
    fn default() -> Self {
        let (sender, receiver) = channel();
        Self {
            sender,
            receiver,
            repaint: None,
            scanning: false,
            polling: false,
            checking: false,
            sweeping: false,
        }
    }
}

impl Tasks {
    pub fn attach(&mut self, ctx: egui::Context) {
        self.repaint = Some(ctx);
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning
    }

    pub fn is_checking(&self) -> bool {
        self.checking
    }

    pub fn is_sweeping(&self) -> bool {
        self.sweeping
    }

    fn channel(&self) -> (Sender<Update>, Option<egui::Context>) {
        (self.sender.clone(), self.repaint.clone())
    }

    fn spawn<F>(&self, name: &str, work: F)
    where
        F: FnOnce(&dyn Fn(Update)) + Send + 'static,
    {
        let (sender, repaint) = self.channel();
        let builder = std::thread::Builder::new().name(format!("rustblox-{name}"));
        let spawned = builder.spawn(move || {
            let emit = move |update: Update| {
                if sender.send(update).is_ok() {
                    if let Some(ctx) = &repaint {
                        ctx.request_repaint();
                    }
                }
            };
            work(&emit);
        });

        if let Err(err) = spawned {
            crate::log_error!("could not start the {name} worker: {err}");
        }
    }

    pub fn scan(&mut self, options: detect::ScanOptions) {
        if self.scanning {
            return;
        }
        self.scanning = true;
        self.spawn("scan", move |emit| {
            let detection = detect::scan(&options);
            emit(Update::Scanned(Box::new(detection)));
        });
    }

    pub fn poll_processes(&mut self) {
        if self.polling {
            return;
        }
        self.polling = true;
        self.spawn("processes", |emit| {
            emit(Update::Processes(process::status()));
        });
    }

    pub fn launch(&self, plan: LaunchPlan, cancel: Arc<AtomicBool>) {
        self.spawn("launch", move |emit| {
            Launcher::run(plan, cancel, &|event| emit(Update::Launch(event)));
        });
    }

    pub fn install(&self, plan: InstallPlan, cancel: Arc<AtomicBool>) {
        self.spawn("install", move |emit| {
            Installer::run(plan, cancel, &|event| emit(Update::Install(event)));
        });
    }

    pub fn check_latest(&mut self, channel: String) {
        if self.checking {
            return;
        }
        self.checking = true;
        self.spawn("version", move |emit| {
            let found = deploy::latest(&channel).map_err(|err| err.to_string());
            emit(Update::Latest(Box::new(found)));
        });
    }

    pub fn sweep(
        &mut self,
        versions_root: PathBuf,
        downloads_root: PathBuf,
        keep_versions: Vec<String>,
        keep_downloads: Vec<String>,
    ) {
        if self.sweeping {
            return;
        }
        self.sweeping = true;
        self.spawn("cleanup", move |emit| {
            let mut sweep = versions::prune_versions(&versions_root, &keep_versions);
            sweep.absorb(versions::tidy_downloads(&downloads_root, &keep_downloads));
            emit(Update::Swept(sweep));
        });
    }

    pub fn remove_version(&mut self, versions_root: PathBuf, folder: String) {
        if self.sweeping {
            return;
        }
        self.sweeping = true;
        self.spawn("remove", move |emit| {
            let dir = versions::version_dir(&versions_root, &folder);
            let mut sweep = Sweep::default();
            match versions::remove_version(&versions_root, &dir) {
                Ok(freed) => {
                    sweep.removed.push(folder);
                    sweep.reclaimed = freed;
                }
                Err(err) => sweep.problems.push(err.to_string()),
            }
            emit(Update::Swept(sweep));
        });
    }

    pub fn drain(&mut self) -> Vec<Update> {
        let mut updates = Vec::new();
        while let Ok(update) = self.receiver.try_recv() {
            match &update {
                Update::Scanned(_) => self.scanning = false,
                Update::Processes(_) => self.polling = false,
                Update::Latest(_) => self.checking = false,
                Update::Swept(_) => self.sweeping = false,
                Update::Launch(_) | Update::Install(_) => {}
            }
            updates.push(update);
        }
        updates
    }
}
