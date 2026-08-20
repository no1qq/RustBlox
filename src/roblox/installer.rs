use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use md5::{Digest, Md5};

use crate::error::{Context, Error, Result};
use crate::platform;
use crate::util::fs;

use super::deploy::{self, Deployment, Package, APP_SETTINGS_BODY, MIRRORS};
use super::install::{
    format_size, InstallSource, Installation, APP_SETTINGS, INCOMPLETE_SUFFIX, PLAYER_EXE,
    PREVIOUS_SUFFIX,
};

type Stop<'a> = &'a (dyn Fn() -> bool + Sync);

const CHUNK: usize = 128 * 1024;
const TIMEOUT: Duration = Duration::from_secs(120);
const MAX_PACKAGE_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_WORKERS: usize = 4;
const ROUNDS: usize = 2;
const BACKOFF: Duration = Duration::from_millis(300);
const PROGRESS_INTERVAL: Duration = Duration::from_millis(40);
const SPACE_MARGIN: u64 = 256 * 1024 * 1024;
const PART_SUFFIX: &str = ".part";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Stage {
    Resolve,
    Manifest,
    Download,
    Extract,
    Finalise,
}

impl Stage {
    pub const ORDER: [Stage; 5] = [
        Stage::Resolve,
        Stage::Manifest,
        Stage::Download,
        Stage::Extract,
        Stage::Finalise,
    ];

    pub fn title(self) -> &'static str {
        match self {
            Stage::Resolve => "Checking for the latest version",
            Stage::Manifest => "Reading the package list",
            Stage::Download => "Downloading Roblox",
            Stage::Extract => "Unpacking files",
            Stage::Finalise => "Finishing up",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StageState {
    Pending,
    Active,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct StageRow {
    pub stage: Stage,
    pub state: StageState,
    pub detail: Option<String>,
}

impl StageRow {
    pub fn pending(stage: Stage) -> Self {
        Self {
            stage,
            state: StageState::Pending,
            detail: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum InstallEvent {
    Stage {
        stage: Stage,
        state: StageState,
        detail: Option<String>,
    },
    Progress {
        done: u64,
        total: u64,
        label: String,
    },
    Finished(std::result::Result<InstallReport, InstallFailure>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallReport {
    pub version: String,
    pub folder: String,
    pub channel: String,
    pub directory: PathBuf,
    pub downloaded: u64,
    pub elapsed: Duration,
    pub already_present: bool,
    pub unknown_packages: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallFailure {
    pub stage: Stage,
    pub message: String,
    pub hint: Option<String>,
    pub cancelled: bool,
}

impl InstallFailure {
    fn from_error(stage: Stage, error: &Error) -> Self {
        Self {
            stage,
            message: error.to_string(),
            hint: error.hint().map(str::to_owned),
            cancelled: false,
        }
    }

    fn cancelled(stage: Stage) -> Self {
        Self {
            stage,
            message: "The install was cancelled.".into(),
            hint: Some("Downloaded packages were kept, so starting again resumes.".into()),
            cancelled: true,
        }
    }
}

#[derive(Clone, Debug)]
pub struct InstallPlan {
    pub channel: String,
    pub versions_root: PathBuf,
    pub downloads_root: PathBuf,
    pub force: bool,
}

pub struct Installer;

impl Installer {
    pub fn run(plan: InstallPlan, cancel: Arc<AtomicBool>, emit: &dyn Fn(InstallEvent)) {
        let started = Instant::now();
        match execute(&plan, &cancel, emit, started) {
            Ok(report) => emit(InstallEvent::Finished(Ok(report))),
            Err(failure) => emit(InstallEvent::Finished(Err(failure))),
        }
    }
}

fn active(emit: &dyn Fn(InstallEvent), stage: Stage) {
    emit(InstallEvent::Stage {
        stage,
        state: StageState::Active,
        detail: None,
    });
}

fn done(emit: &dyn Fn(InstallEvent), stage: Stage, detail: impl Into<String>) {
    emit(InstallEvent::Stage {
        stage,
        state: StageState::Done,
        detail: Some(detail.into()),
    });
}

fn failed(emit: &dyn Fn(InstallEvent), stage: Stage, detail: impl Into<String>) {
    emit(InstallEvent::Stage {
        stage,
        state: StageState::Failed,
        detail: Some(detail.into()),
    });
}

fn check(cancel: &AtomicBool, stage: Stage) -> std::result::Result<(), InstallFailure> {
    if cancel.load(Ordering::Relaxed) {
        Err(InstallFailure::cancelled(stage))
    } else {
        Ok(())
    }
}

fn execute(
    plan: &InstallPlan,
    cancel: &AtomicBool,
    emit: &dyn Fn(InstallEvent),
    started: Instant,
) -> std::result::Result<InstallReport, InstallFailure> {
    active(emit, Stage::Resolve);
    let deployment = deploy::latest(&plan.channel).map_err(|err| {
        failed(emit, Stage::Resolve, err.to_string());
        InstallFailure {
            stage: Stage::Resolve,
            message: err.to_string(),
            hint: Some("Check your internet connection and try again.".into()),
            cancelled: false,
        }
    })?;
    done(
        emit,
        Stage::Resolve,
        format!("{} on {}", deployment.version, deployment.channel),
    );

    let target = plan.versions_root.join(&deployment.folder);
    if !plan.force && is_complete(&target) {
        for stage in [Stage::Manifest, Stage::Download, Stage::Extract] {
            done(emit, stage, "already installed");
        }
        done(emit, Stage::Finalise, "nothing to do");
        return Ok(InstallReport {
            version: deployment.version,
            folder: deployment.folder,
            channel: deployment.channel,
            directory: target,
            downloaded: 0,
            elapsed: started.elapsed(),
            already_present: true,
            unknown_packages: Vec::new(),
        });
    }
    check(cancel, Stage::Manifest)?;

    active(emit, Stage::Manifest);
    let manifest = deploy::manifest(&deployment).map_err(|err| {
        failed(emit, Stage::Manifest, err.to_string());
        InstallFailure::from_error(Stage::Manifest, &err)
    })?;
    let packages: Vec<Package> = manifest.extractable().cloned().collect();
    let unknown: Vec<String> = manifest
        .packages
        .iter()
        .filter(|package| {
            package.name.to_ascii_lowercase().ends_with(".zip")
                && package.target().is_none()
                && !deploy::is_deliberately_skipped(&package.name)
        })
        .map(|package| package.name.clone())
        .collect();
    done(
        emit,
        Stage::Manifest,
        format!(
            "{} packages, {}",
            packages.len(),
            format_size(manifest.download_size())
        ),
    );

    if let Err(err) = ensure_space(
        &plan.downloads_root,
        &plan.versions_root,
        manifest.download_size(),
        manifest.install_size(),
    ) {
        failed(emit, Stage::Download, err.to_string());
        return Err(InstallFailure {
            stage: Stage::Download,
            message: err.to_string(),
            hint: Some("Free some space on that drive and start the install again.".into()),
            cancelled: false,
        });
    }
    check(cancel, Stage::Download)?;

    let staging = plan.downloads_root.join(&deployment.folder);
    fs::ensure_dir(&staging).map_err(|err| {
        failed(emit, Stage::Download, err.to_string());
        InstallFailure::from_error(Stage::Download, &err)
    })?;

    active(emit, Stage::Download);
    let fetched_bytes = match download_all(&deployment, &packages, &staging, cancel, emit) {
        Ok(bytes) => bytes,
        Err(err) => {
            if cancel.load(Ordering::Relaxed) {
                failed(emit, Stage::Download, "cancelled");
                return Err(InstallFailure::cancelled(Stage::Download));
            }
            failed(emit, Stage::Download, err.to_string());
            return Err(InstallFailure {
                stage: Stage::Download,
                message: err.to_string(),
                hint: Some("The download can be resumed by starting the install again.".into()),
                cancelled: false,
            });
        }
    };
    done(
        emit,
        Stage::Download,
        format!("{} fetched", format_size(fetched_bytes)),
    );
    check(cancel, Stage::Extract)?;

    let staged = sibling(&target, INCOMPLETE_SUFFIX);
    let _ = std::fs::remove_dir_all(&staged);

    active(emit, Stage::Extract);
    let total_unpacked = manifest.install_size();
    if let Err(err) = extract_all(&packages, &staging, &staged, total_unpacked, cancel, emit) {
        let _ = std::fs::remove_dir_all(&staged);
        if cancel.load(Ordering::Relaxed) {
            failed(emit, Stage::Extract, "cancelled");
            return Err(InstallFailure::cancelled(Stage::Extract));
        }
        failed(emit, Stage::Extract, err.to_string());
        return Err(InstallFailure {
            stage: Stage::Extract,
            message: err.to_string(),
            hint: Some("Check that there is enough free disk space, then try again.".into()),
            cancelled: false,
        });
    }
    done(
        emit,
        Stage::Extract,
        format!("{} written", format_size(total_unpacked)),
    );

    if let Err(failure) = check(cancel, Stage::Finalise) {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(failure);
    }

    active(emit, Stage::Finalise);
    if let Err(err) = fs::write_atomic(&staged.join(APP_SETTINGS), APP_SETTINGS_BODY.as_bytes()) {
        let _ = std::fs::remove_dir_all(&staged);
        failed(emit, Stage::Finalise, err.to_string());
        return Err(InstallFailure::from_error(Stage::Finalise, &err));
    }

    if !staged.join(PLAYER_EXE).is_file() {
        let _ = std::fs::remove_dir_all(&staged);
        let message = format!("{PLAYER_EXE} was missing after unpacking");
        failed(emit, Stage::Finalise, message.clone());
        return Err(InstallFailure {
            stage: Stage::Finalise,
            message,
            hint: Some(
                "Roblox may have changed its package layout. Report this so the map can be updated."
                    .into(),
            ),
            cancelled: false,
        });
    }

    if let Err(err) = swap_into_place(&staged, &target) {
        let _ = std::fs::remove_dir_all(&staged);
        failed(emit, Stage::Finalise, err.to_string());
        return Err(InstallFailure {
            stage: Stage::Finalise,
            message: err.to_string(),
            hint: Some("Close Roblox if it is running, then try again.".into()),
            cancelled: false,
        });
    }

    done(emit, Stage::Finalise, "ready to launch");

    Ok(InstallReport {
        version: deployment.version,
        folder: deployment.folder,
        channel: deployment.channel,
        directory: target,
        downloaded: fetched_bytes,
        elapsed: started.elapsed(),
        already_present: false,
        unknown_packages: unknown,
    })
}

fn is_complete(dir: &Path) -> bool {
    Installation::from_version_dir(dir, InstallSource::Ours)
        .map(|install| install.integrity().is_ok())
        .unwrap_or(false)
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{name}{suffix}"))
}

fn volume(path: &Path) -> Option<String> {
    path.components()
        .next()
        .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
}

fn ensure_space(
    downloads: &Path,
    versions: &Path,
    download_bytes: u64,
    install_bytes: u64,
) -> Result<()> {
    let shared = volume(downloads).is_some() && volume(downloads) == volume(versions);
    let checks: Vec<(&Path, u64)> = if shared {
        vec![(versions, download_bytes.saturating_add(install_bytes))]
    } else {
        vec![(downloads, download_bytes), (versions, install_bytes)]
    };

    for (path, wanted) in checks {
        let needed = wanted.saturating_add(SPACE_MARGIN);
        let Some(free) = platform::free_space(path) else {
            continue;
        };
        if free < needed {
            return Err(Error::invalid(format!(
                "{} needs {} free to install Roblox but only {} is available",
                path.display(),
                format_size(needed),
                format_size(free)
            )));
        }
    }

    Ok(())
}

fn swap_into_place(staged: &Path, target: &Path) -> Result<()> {
    if let Some(parent) = target.parent() {
        fs::ensure_dir(parent)?;
    }

    if !target.exists() {
        return std::fs::rename(staged, target)
            .map_err(|err| Error::io(format!("could not move {}", staged.display()), err));
    }

    let retired = sibling(target, PREVIOUS_SUFFIX);
    let _ = std::fs::remove_dir_all(&retired);
    std::fs::rename(target, &retired)
        .map_err(|err| Error::io(format!("could not move {} aside", target.display()), err))?;

    match std::fs::rename(staged, target) {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&retired);
            Ok(())
        }
        Err(err) => {
            let _ = std::fs::rename(&retired, target);
            Err(Error::io(
                format!("could not move the new files into {}", target.display()),
                err,
            ))
        }
    }
}

fn workers(jobs: usize) -> usize {
    let cores = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(2);
    cores.clamp(1, MAX_WORKERS).min(jobs.max(1))
}

enum Note {
    Bytes { index: usize, done: u64 },
    Finished { index: usize, fetched: u64 },
    Failed(Error),
}

fn download_all(
    deployment: &Deployment,
    packages: &[Package],
    staging: &Path,
    cancel: &AtomicBool,
    emit: &dyn Fn(InstallEvent),
) -> Result<u64> {
    if packages.is_empty() {
        return Ok(0);
    }

    let total: u64 = packages.iter().map(|package| package.packed_size).sum();
    let counter = AtomicUsize::new(0);
    let flag = AtomicBool::new(false);
    let next = &counter;
    let abort = &flag;
    let stop = move || cancel.load(Ordering::Relaxed) || abort.load(Ordering::Relaxed);
    let client = agent();

    let mut have = vec![0u64; packages.len()];
    let mut fetched = 0u64;
    let mut complete = 0usize;
    let mut failure: Option<Error> = None;

    std::thread::scope(|scope| {
        let (sender, notes) = std::sync::mpsc::channel::<Note>();
        let stop: Stop = &stop;

        for _ in 0..workers(packages.len()) {
            let sender = sender.clone();
            let client = client.clone();
            scope.spawn(move || loop {
                if stop() {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(package) = packages.get(index) else {
                    break;
                };

                let path = staging.join(&package.name);
                if file_matches(&path, &package.checksum) {
                    let _ = sender.send(Note::Finished { index, fetched: 0 });
                    continue;
                }

                let report = |done: u64| {
                    let _ = sender.send(Note::Bytes { index, done });
                };

                match fetch_package(&client, deployment, package, &path, stop, &report) {
                    Ok(bytes) => {
                        let _ = sender.send(Note::Finished {
                            index,
                            fetched: bytes,
                        });
                    }
                    Err(err) => {
                        let _ = sender.send(Note::Failed(err));
                        break;
                    }
                }
            });
        }
        drop(sender);

        let mut last = Instant::now() - PROGRESS_INTERVAL;
        for note in notes {
            match note {
                Note::Bytes { index, done } => {
                    have[index] = done.min(packages[index].packed_size);
                    if last.elapsed() < PROGRESS_INTERVAL {
                        continue;
                    }
                }
                Note::Finished {
                    index,
                    fetched: got,
                } => {
                    have[index] = packages[index].packed_size;
                    fetched = fetched.saturating_add(got);
                    complete += 1;
                }
                Note::Failed(err) => {
                    if failure.is_none() {
                        failure = Some(err);
                    }
                    abort.store(true, Ordering::Relaxed);
                    continue;
                }
            }

            last = Instant::now();
            emit(InstallEvent::Progress {
                done: have.iter().sum(),
                total,
                label: format!("{complete} of {} packages", packages.len()),
            });
        }
    });

    match failure {
        Some(err) => Err(err),
        None if cancel.load(Ordering::Relaxed) => Err(Error::invalid("cancelled")),
        None => Ok(fetched),
    }
}

fn extract_all(
    packages: &[Package],
    staging: &Path,
    root: &Path,
    total: u64,
    cancel: &AtomicBool,
    emit: &dyn Fn(InstallEvent),
) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    let counter = AtomicUsize::new(0);
    let flag = AtomicBool::new(false);
    let next = &counter;
    let abort = &flag;
    let stop = move || cancel.load(Ordering::Relaxed) || abort.load(Ordering::Relaxed);

    let mut unpacked = 0u64;
    let mut complete = 0usize;
    let mut failure: Option<Error> = None;

    std::thread::scope(|scope| {
        let (sender, notes) = std::sync::mpsc::channel::<Note>();
        let stop: Stop = &stop;

        for _ in 0..workers(packages.len()) {
            let sender = sender.clone();
            scope.spawn(move || loop {
                if stop() {
                    break;
                }
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(package) = packages.get(index) else {
                    break;
                };
                let Some(relative) = package.target() else {
                    let _ = sender.send(Note::Finished { index, fetched: 0 });
                    continue;
                };

                let mut destination = root.to_path_buf();
                for part in relative.split('/').filter(|part| !part.is_empty()) {
                    destination.push(part);
                }

                match extract_package(&staging.join(&package.name), &destination, stop) {
                    Ok(()) => {
                        let _ = sender.send(Note::Finished { index, fetched: 0 });
                    }
                    Err(err) => {
                        let _ = sender.send(Note::Failed(err));
                        break;
                    }
                }
            });
        }
        drop(sender);

        for note in notes {
            match note {
                Note::Finished { index, .. } => {
                    unpacked = unpacked.saturating_add(packages[index].unpacked_size);
                    complete += 1;
                }
                Note::Failed(err) => {
                    if failure.is_none() {
                        failure = Some(err);
                    }
                    abort.store(true, Ordering::Relaxed);
                    continue;
                }
                Note::Bytes { .. } => continue,
            }

            emit(InstallEvent::Progress {
                done: unpacked,
                total,
                label: format!("{complete} of {} packages", packages.len()),
            });
        }
    });

    match failure {
        Some(err) => Err(err),
        None if cancel.load(Ordering::Relaxed) => Err(Error::invalid("cancelled")),
        None => Ok(()),
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

pub fn checksum_of(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let mut hasher = Md5::new();
    feed(&mut hasher, file, u64::MAX).ok()?;
    Some(hex(&hasher.finalize()))
}

fn feed(hasher: &mut Md5, source: std::fs::File, limit: u64) -> std::io::Result<u64> {
    let mut reader = std::io::BufReader::new(source.take(limit));
    let mut buffer = vec![0u8; CHUNK];
    let mut read_total = 0u64;

    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        read_total = read_total.saturating_add(read as u64);
    }

    Ok(read_total)
}

fn file_matches(path: &Path, checksum: &str) -> bool {
    checksum_of(path)
        .map(|actual| actual.eq_ignore_ascii_case(checksum))
        .unwrap_or(false)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("RustBlox/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn fetch_package(
    client: &ureq::Agent,
    deployment: &Deployment,
    package: &Package,
    path: &Path,
    stop: Stop,
    progress: &dyn Fn(u64),
) -> Result<u64> {
    let part = sibling(path, PART_SUFFIX);
    let mut last: Option<Error> = None;
    let attempts: Vec<&str> = MIRRORS
        .iter()
        .cycle()
        .take(MIRRORS.len() * ROUNDS)
        .copied()
        .collect();

    for (attempt, mirror) in attempts.iter().enumerate() {
        if stop() {
            return Err(Error::invalid("cancelled"));
        }

        let url = deployment.package_url(mirror, &package.name);
        match download_to(client, &url, &part, package, stop, progress) {
            Ok(bytes) => {
                let _ = std::fs::remove_file(path);
                std::fs::rename(&part, path)
                    .map_err(|err| Error::io(format!("could not store {}", package.name), err))?;
                return Ok(bytes);
            }
            Err(err) => last = Some(err),
        }

        if attempt + 1 < attempts.len() && !stop() {
            std::thread::sleep(BACKOFF * (attempt as u32 + 1));
        }
    }

    Err(last.unwrap_or_else(|| Error::invalid(format!("{} could not be downloaded", package.name))))
}

fn download_to(
    client: &ureq::Agent,
    url: &str,
    part: &Path,
    package: &Package,
    stop: Stop,
    progress: &dyn Fn(u64),
) -> Result<u64> {
    if package.packed_size > MAX_PACKAGE_BYTES {
        return Err(Error::invalid(format!(
            "{} claims an implausible size and was refused",
            package.name
        )));
    }

    let on_disk = std::fs::metadata(part)
        .map(|meta| meta.len())
        .unwrap_or_default();
    let resume = if on_disk > 0 && on_disk < package.packed_size {
        on_disk
    } else {
        0
    };

    let mut request = client.get(url);
    if resume > 0 {
        request = request.header("Range", &format!("bytes={resume}-"));
    }

    let mut response = match request.call() {
        Ok(response) => response,
        Err(err) => {
            if resume > 0 && matches!(err, ureq::Error::StatusCode(_)) {
                let _ = std::fs::remove_file(part);
            }
            return Err(Error::invalid(format!("{url} could not be reached: {err}")));
        }
    };

    let continuing = resume > 0 && response.status().as_u16() == 206;
    let mut hasher = Md5::new();
    let mut written = 0u64;

    let mut file = if continuing {
        let existing = std::fs::File::open(part).ctx_path("could not reopen", part)?;
        written = feed(&mut hasher, existing, resume)
            .map_err(|err| Error::io(format!("could not read {}", part.display()), err))?;
        progress(written.min(package.packed_size));
        std::fs::OpenOptions::new()
            .append(true)
            .open(part)
            .ctx_path("could not append to", part)?
    } else {
        std::fs::File::create(part).ctx_path("could not create", part)?
    };

    let mut reader = response.body_mut().as_reader();
    let mut buffer = vec![0u8; CHUNK];

    loop {
        if stop() {
            return Err(Error::invalid("cancelled"));
        }

        let read = reader
            .read(&mut buffer)
            .map_err(|err| Error::io(format!("could not read from {url}"), err))?;
        if read == 0 {
            break;
        }

        written = written.saturating_add(read as u64);
        if written > MAX_PACKAGE_BYTES {
            return Err(Error::invalid(format!(
                "{} sent more data than expected",
                package.name
            )));
        }

        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .ctx_path("could not write to", part)?;
        progress(written.min(package.packed_size));
    }

    file.sync_all().ctx_path("could not flush", part)?;
    drop(file);

    let actual = hex(&hasher.finalize());
    if !actual.eq_ignore_ascii_case(&package.checksum) {
        let _ = std::fs::remove_file(part);
        return Err(Error::invalid(format!(
            "{} failed its checksum, the download was corrupted",
            package.name
        )));
    }

    Ok(written)
}

fn extract_package(archive: &Path, destination: &Path, stop: Stop) -> Result<()> {
    let file = std::fs::File::open(archive).ctx_path("could not open", archive)?;
    let mut zip = zip::ZipArchive::new(std::io::BufReader::new(file)).map_err(|err| {
        Error::invalid(format!(
            "{} is not a readable zip: {err}",
            archive.display()
        ))
    })?;

    fs::ensure_dir(destination)?;

    for index in 0..zip.len() {
        if stop() {
            return Err(Error::invalid("cancelled"));
        }

        let mut entry = zip
            .by_index(index)
            .map_err(|err| Error::invalid(format!("entry {index} could not be read: {err}")))?;

        let Some(relative) = entry.enclosed_name() else {
            return Err(Error::invalid(format!(
                "{} contains an unsafe path and was refused",
                archive.display()
            )));
        };

        let out = destination.join(&relative);
        if entry.is_dir() {
            fs::ensure_dir(&out)?;
            continue;
        }

        if let Some(parent) = out.parent() {
            fs::ensure_dir(parent)?;
        }

        let mut target = std::fs::File::create(&out).ctx_path("could not create", &out)?;
        std::io::copy(&mut entry, &mut target)
            .map_err(|err| Error::io(format!("could not write {}", out.display()), err))?;
    }

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_match_the_manifest_format() {
        let mut hasher = Md5::new();
        hasher.update(b"rustblox");
        let digest = hex(&hasher.finalize());

        assert_eq!(digest.len(), 32);
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(digest, digest.to_lowercase());
    }

    #[test]
    fn checksums_a_real_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("payload.bin");
        std::fs::write(&path, b"hello world").unwrap();

        assert_eq!(
            checksum_of(&path).unwrap(),
            "5eb63bbbe01eeed093cb22bb8f5acdc3"
        );
        assert!(file_matches(&path, "5EB63BBBE01EEED093CB22BB8F5ACDC3"));
        assert!(!file_matches(&path, "0".repeat(32).as_str()));
    }

    #[test]
    fn a_missing_file_never_matches() {
        let dir = tempfile::tempdir().unwrap();
        assert!(checksum_of(&dir.path().join("nope")).is_none());
        assert!(!file_matches(&dir.path().join("nope"), "abc"));
    }

    #[test]
    fn extraction_refuses_a_traversal_entry() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("evil.zip");

        let file = std::fs::File::create(&archive).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("../escaped.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"nope").unwrap();
        writer.finish().unwrap();

        let out = dir.path().join("out");
        let result = extract_package(&archive, &out, &|| false);

        assert!(result.is_err());
        assert!(!dir.path().join("escaped.txt").exists());
    }

    #[test]
    fn extraction_writes_nested_entries() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("good.zip");

        let file = std::fs::File::create(&archive).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("a/b/c.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"payload").unwrap();
        writer.finish().unwrap();

        let out = dir.path().join("out");
        extract_package(&archive, &out, &|| false).unwrap();

        assert_eq!(
            std::fs::read_to_string(out.join("a").join("b").join("c.txt")).unwrap(),
            "payload"
        );
    }

    #[test]
    fn extraction_stops_when_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join("stop.zip");

        let file = std::fs::File::create(&archive).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("file.txt", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"payload").unwrap();
        writer.finish().unwrap();

        let result = extract_package(&archive, &dir.path().join("out"), &|| true);
        assert!(result.is_err());
    }

    fn zipped(dir: &Path, name: &str, entry: &str, body: &[u8]) -> Package {
        let archive = dir.join(name);
        let file = std::fs::File::create(&archive).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file(entry, zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(body).unwrap();
        writer.finish().unwrap();

        Package {
            name: name.into(),
            checksum: checksum_of(&archive).unwrap(),
            packed_size: std::fs::metadata(&archive).unwrap().len(),
            unpacked_size: body.len() as u64,
        }
    }

    #[test]
    fn siblings_keep_the_whole_folder_name() {
        assert_eq!(
            sibling(Path::new(r"C:\a\version-abc.1"), ".incomplete"),
            PathBuf::from(r"C:\a\version-abc.1.incomplete")
        );
    }

    #[test]
    fn a_half_finished_folder_is_not_treated_as_installed() {
        let dir = tempfile::tempdir().unwrap();
        let version = dir.path().join("version-x");
        std::fs::create_dir_all(&version).unwrap();
        std::fs::write(version.join(PLAYER_EXE), b"stub").unwrap();
        std::fs::write(version.join(APP_SETTINGS), b"stub").unwrap();

        assert!(!is_complete(&version));

        for folder in ["content", "ExtraContent", "shaders"] {
            std::fs::create_dir_all(version.join(folder)).unwrap();
        }

        assert!(is_complete(&version));
    }

    #[test]
    fn swapping_replaces_an_existing_folder() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("version-x");
        let staged = sibling(&target, INCOMPLETE_SUFFIX);

        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("old.txt"), b"old").unwrap();
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::write(staged.join("new.txt"), b"new").unwrap();

        swap_into_place(&staged, &target).unwrap();

        assert!(target.join("new.txt").is_file());
        assert!(!target.join("old.txt").exists());
        assert!(!staged.exists());
        assert!(!sibling(&target, PREVIOUS_SUFFIX).exists());
    }

    #[test]
    fn swapping_into_a_fresh_folder_works() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("nested").join("version-y");
        let staged = dir.path().join("nested").join("staged");

        std::fs::create_dir_all(&staged).unwrap();
        std::fs::write(staged.join("new.txt"), b"new").unwrap();

        swap_into_place(&staged, &target).unwrap();

        assert!(target.join("new.txt").is_file());
    }

    #[test]
    fn an_impossible_space_requirement_is_refused_early() {
        let dir = tempfile::tempdir().unwrap();
        let result = ensure_space(dir.path(), dir.path(), u64::MAX / 4, u64::MAX / 4);
        if platform::free_space(dir.path()).is_some() {
            assert!(result.is_err());
        }
    }

    #[test]
    fn a_reasonable_space_requirement_is_allowed() {
        let dir = tempfile::tempdir().unwrap();
        ensure_space(dir.path(), dir.path(), 0, 0).unwrap();
    }

    #[test]
    fn extraction_lays_every_package_out_under_one_root() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("downloads");
        std::fs::create_dir_all(&staging).unwrap();

        let packages = vec![
            zipped(&staging, "RobloxApp.zip", "RobloxPlayerBeta.exe", b"player"),
            zipped(&staging, "shaders.zip", "shaders.pack", b"shaders"),
            zipped(&staging, "content-fonts.zip", "font.ttf", b"font"),
        ];

        let root = dir.path().join("version-z");
        let cancel = AtomicBool::new(false);
        extract_all(&packages, &staging, &root, 17, &cancel, &|_| {}).unwrap();

        assert_eq!(
            std::fs::read(root.join(PLAYER_EXE)).unwrap(),
            b"player".to_vec()
        );
        assert!(root.join("shaders").join("shaders.pack").is_file());
        assert!(root
            .join("content")
            .join("fonts")
            .join("font.ttf")
            .is_file());
    }

    #[test]
    fn extraction_reports_a_broken_package() {
        let dir = tempfile::tempdir().unwrap();
        let staging = dir.path().join("downloads");
        std::fs::create_dir_all(&staging).unwrap();
        std::fs::write(staging.join("shaders.zip"), b"not a zip").unwrap();

        let packages = vec![Package {
            name: "shaders.zip".into(),
            checksum: String::new(),
            packed_size: 9,
            unpacked_size: 9,
        }];

        let cancel = AtomicBool::new(false);
        let result = extract_all(
            &packages,
            &staging,
            &dir.path().join("out"),
            9,
            &cancel,
            &|_| {},
        );

        assert!(result.is_err());
    }
}

#[cfg(test)]
mod live {
    use super::*;
    use crate::roblox::deploy;

    #[test]
    #[ignore = "reaches the Roblox CDN"]
    fn downloads_verifies_and_extracts_a_real_package() {
        let deployment = deploy::latest(deploy::DEFAULT_CHANNEL).expect("version lookup");
        assert!(deployment.folder.starts_with("version-"));
        println!("channel {} -> {}", deployment.channel, deployment.version);

        let manifest = deploy::manifest(&deployment).expect("manifest");
        let packages: Vec<_> = manifest.extractable().cloned().collect();
        assert!(packages.len() > 10, "expected a full package list");
        println!(
            "{} packages, {} download, {} installed",
            packages.len(),
            super::super::install::format_size(manifest.download_size()),
            super::super::install::format_size(manifest.install_size())
        );

        for package in &packages {
            assert!(
                package.target().is_some(),
                "unmapped package {}",
                package.name
            );
        }

        let smallest = packages
            .iter()
            .min_by_key(|package| package.packed_size)
            .expect("a package");
        println!(
            "fetching {} ({})",
            smallest.name,
            super::super::install::format_size(smallest.packed_size)
        );

        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join(&smallest.name);

        let bytes = fetch_package(
            &agent(),
            &deployment,
            smallest,
            &archive,
            &|| false,
            &|_| {},
        )
        .expect("download and checksum");
        assert_eq!(bytes, smallest.packed_size);

        let out = dir.path().join("extracted");
        extract_package(&archive, &out, &|| false).expect("extract");
        assert!(
            out.read_dir().unwrap().next().is_some(),
            "nothing extracted"
        );
        println!("verified {} into {}", smallest.name, out.display());
    }

    #[test]
    #[ignore = "reaches the Roblox CDN"]
    fn a_partial_download_resumes_from_where_it_stopped() {
        let deployment = deploy::latest(deploy::DEFAULT_CHANNEL).expect("version lookup");
        let manifest = deploy::manifest(&deployment).expect("manifest");
        let package = manifest
            .extractable()
            .filter(|package| package.packed_size > 64 * 1024)
            .min_by_key(|package| package.packed_size)
            .expect("a package worth resuming")
            .clone();

        let dir = tempfile::tempdir().unwrap();
        let archive = dir.path().join(&package.name);
        let client = agent();

        fetch_package(&client, &deployment, &package, &archive, &|| false, &|_| {})
            .expect("first download");

        let whole = std::fs::read(&archive).unwrap();
        let half = whole.len() / 2;
        std::fs::write(sibling(&archive, PART_SUFFIX), &whole[..half]).unwrap();
        std::fs::remove_file(&archive).unwrap();

        let first = std::sync::Mutex::new(None);
        let bytes = fetch_package(
            &client,
            &deployment,
            &package,
            &archive,
            &|| false,
            &|done| {
                let mut first = first.lock().unwrap();
                if first.is_none() {
                    *first = Some(done);
                }
            },
        )
        .expect("resumed download");

        assert!(file_matches(&archive, &package.checksum));
        assert!(!sibling(&archive, PART_SUFFIX).exists());
        assert_eq!(bytes, package.packed_size);
        assert_eq!(
            first.into_inner().unwrap(),
            Some(half as u64),
            "the CDN did not honour the range request, {} was refetched in full",
            package.name
        );
        println!(
            "{}: resumed at {} of {} bytes",
            package.name, half, package.packed_size
        );
    }

    #[test]
    #[ignore = "reaches the Roblox CDN"]
    fn every_published_package_has_a_known_destination() {
        let deployment = deploy::latest(deploy::DEFAULT_CHANNEL).expect("version lookup");
        let manifest = deploy::manifest(&deployment).expect("manifest");

        let unmapped: Vec<&str> = manifest
            .packages
            .iter()
            .filter(|package| {
                package.name.to_ascii_lowercase().ends_with(".zip")
                    && deploy::package_target(&package.name).is_none()
                    && !deploy::is_deliberately_skipped(&package.name)
            })
            .map(|package| package.name.as_str())
            .collect();

        assert!(
            unmapped.is_empty(),
            "Roblox published packages RustBlox does not know where to put: {unmapped:?}"
        );
    }
}
