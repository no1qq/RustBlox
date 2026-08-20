use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{Context, Error, Result};
use crate::util::version;

pub const REPOSITORY: &str = "https://github.com/no1qq/RustBlox";
pub const ASSET: &str = "RustBlox.exe";

const RELEASES_API: &str = "https://api.github.com/repos/no1qq/RustBlox/releases?per_page=50";
const TIMEOUT: Duration = Duration::from_secs(30);
const MAX_LISTING_BYTES: usize = 4 * 1024 * 1024;
const MAX_EXE_BYTES: u64 = 256 * 1024 * 1024;
const CHUNK: usize = 64 * 1024;
const NEW_SUFFIX: &str = ".new";
const OLD_SUFFIX: &str = ".old";

pub type Stop<'a> = &'a (dyn Fn() -> bool + Sync);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub tag: String,
    pub version: String,
    pub url: String,
    pub size: u64,
    pub page: String,
}

impl Release {
    pub fn is_newer_than(&self, current: &str) -> bool {
        version::is_newer(&self.version, current)
    }
}

pub fn current_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("RustBlox/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{name}{suffix}"))
}

pub fn parse_releases(body: &str) -> Result<Vec<Release>> {
    let value: serde_json::Value = serde_json::from_str(body)
        .map_err(|err| Error::invalid(format!("the release list could not be read: {err}")))?;

    let Some(entries) = value.as_array() else {
        return Err(Error::invalid("the release list was not a list"));
    };

    let mut releases = Vec::new();

    for entry in entries {
        if entry["draft"].as_bool().unwrap_or(false)
            || entry["prerelease"].as_bool().unwrap_or(false)
        {
            continue;
        }

        let Some(tag) = entry["tag_name"].as_str() else {
            continue;
        };
        let parsed = version::parse(tag);
        if parsed.is_empty() {
            continue;
        }

        let Some(assets) = entry["assets"].as_array() else {
            continue;
        };
        let Some(asset) = assets.iter().find(|asset| {
            asset["name"]
                .as_str()
                .map(|name| name.eq_ignore_ascii_case(ASSET))
                .unwrap_or(false)
        }) else {
            continue;
        };
        let Some(url) = asset["browser_download_url"].as_str() else {
            continue;
        };

        releases.push(Release {
            tag: tag.to_owned(),
            version: tag.trim_start_matches(['v', 'V']).to_owned(),
            url: url.to_owned(),
            size: asset["size"].as_u64().unwrap_or(0),
            page: entry["html_url"].as_str().unwrap_or(REPOSITORY).to_owned(),
        });
    }

    releases.sort_by(|a, b| version::compare(&version::parse(&b.tag), &version::parse(&a.tag)));
    Ok(releases)
}

pub fn newest() -> Result<Option<Release>> {
    let mut response = agent()
        .get(RELEASES_API)
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|err| Error::invalid(format!("GitHub could not be reached: {err}")))?;

    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_LISTING_BYTES as u64)
        .read_to_string()
        .map_err(|err| Error::invalid(format!("the release list could not be read: {err}")))?;

    Ok(parse_releases(&body)?.into_iter().next())
}

pub fn available(current: &str) -> Result<Option<Release>> {
    Ok(newest()?.filter(|release| release.is_newer_than(current)))
}

pub fn staged_path(exe: &Path) -> PathBuf {
    sibling(exe, NEW_SUFFIX)
}

pub fn retired_path(exe: &Path) -> PathBuf {
    sibling(exe, OLD_SUFFIX)
}

pub fn clear_retired(exe: &Path) {
    let _ = std::fs::remove_file(retired_path(exe));
    let _ = std::fs::remove_file(staged_path(exe));
}

pub fn looks_like_a_program(bytes: &[u8]) -> bool {
    bytes.starts_with(b"MZ")
}

pub fn download(release: &Release, into: &Path, stop: Stop, progress: &dyn Fn(u64)) -> Result<u64> {
    if release.size > MAX_EXE_BYTES {
        return Err(Error::invalid(format!(
            "{} is {} which is far larger than a RustBlox build should be",
            ASSET,
            crate::roblox::install::format_size(release.size)
        )));
    }

    let mut response = agent()
        .get(&release.url)
        .call()
        .map_err(|err| Error::invalid(format!("{} could not be reached: {err}", release.url)))?;

    let mut reader = response.body_mut().as_reader();
    let mut file = std::fs::File::create(into).ctx_path("could not create", into)?;
    let mut buffer = vec![0u8; CHUNK];
    let mut written = 0u64;
    let mut head = Vec::new();

    loop {
        if stop() {
            drop(file);
            let _ = std::fs::remove_file(into);
            return Err(Error::invalid("cancelled"));
        }

        let read = reader
            .read(&mut buffer)
            .map_err(|err| Error::io(format!("could not read from {}", release.url), err))?;
        if read == 0 {
            break;
        }

        if head.len() < 2 {
            head.extend_from_slice(&buffer[..read.min(2)]);
        }

        written = written.saturating_add(read as u64);
        if written > MAX_EXE_BYTES {
            drop(file);
            let _ = std::fs::remove_file(into);
            return Err(Error::invalid(format!(
                "{ASSET} sent more data than expected"
            )));
        }

        file.write_all(&buffer[..read])
            .ctx_path("could not write to", into)?;
        progress(written);
    }

    file.sync_all().ctx_path("could not flush", into)?;
    drop(file);

    if !looks_like_a_program(&head) {
        let _ = std::fs::remove_file(into);
        return Err(Error::invalid(format!(
            "what GitHub sent for {ASSET} was not a Windows program"
        )));
    }

    if release.size > 0 && written != release.size {
        let _ = std::fs::remove_file(into);
        return Err(Error::invalid(format!(
            "{ASSET} arrived incomplete, {written} bytes of {}",
            release.size
        )));
    }

    Ok(written)
}

pub fn swap_in(staged: &Path, exe: &Path) -> Result<()> {
    if !staged.is_file() {
        return Err(Error::invalid("the downloaded build is missing"));
    }

    let retired = retired_path(exe);
    let _ = std::fs::remove_file(&retired);
    std::fs::rename(exe, &retired)
        .map_err(|err| Error::io(format!("could not move {} aside", exe.display()), err))?;

    match std::fs::rename(staged, exe) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = std::fs::rename(&retired, exe);
            Err(Error::io(
                format!("could not put the new build at {}", exe.display()),
                err,
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTING: &str = r#"[
        {
            "tag_name": "v1.0.1",
            "draft": false,
            "prerelease": false,
            "html_url": "https://github.com/no1qq/RustBlox/releases/tag/v1.0.1",
            "assets": [
                {"name": "RustBlox.exe", "browser_download_url": "https://example.invalid/1.0.1", "size": 120}
            ]
        },
        {
            "tag_name": "v2.3.1",
            "draft": false,
            "prerelease": false,
            "html_url": "https://github.com/no1qq/RustBlox/releases/tag/v2.3.1",
            "assets": [
                {"name": "rustblox.exe", "browser_download_url": "https://example.invalid/2.3.1", "size": 130}
            ]
        },
        {
            "tag_name": "v9.9.9",
            "draft": true,
            "prerelease": false,
            "html_url": "https://example.invalid/draft",
            "assets": [
                {"name": "RustBlox.exe", "browser_download_url": "https://example.invalid/draft", "size": 1}
            ]
        },
        {
            "tag_name": "v8.8.8",
            "draft": false,
            "prerelease": true,
            "html_url": "https://example.invalid/pre",
            "assets": [
                {"name": "RustBlox.exe", "browser_download_url": "https://example.invalid/pre", "size": 1}
            ]
        },
        {
            "tag_name": "v7.7.7",
            "draft": false,
            "prerelease": false,
            "html_url": "https://example.invalid/no-asset",
            "assets": [
                {"name": "notes.txt", "browser_download_url": "https://example.invalid/notes", "size": 1}
            ]
        }
    ]"#;

    #[test]
    fn the_highest_tag_with_an_attached_build_wins() {
        let releases = parse_releases(LISTING).unwrap();
        assert_eq!(releases.len(), 2);
        assert_eq!(releases[0].tag, "v2.3.1");
        assert_eq!(releases[0].version, "2.3.1");
        assert_eq!(releases[0].url, "https://example.invalid/2.3.1");
        assert_eq!(releases[0].size, 130);
    }

    #[test]
    fn drafts_prereleases_and_assetless_tags_are_skipped() {
        let releases = parse_releases(LISTING).unwrap();
        let tags: Vec<&str> = releases
            .iter()
            .map(|release| release.tag.as_str())
            .collect();
        assert_eq!(tags, ["v2.3.1", "v1.0.1"]);
    }

    #[test]
    fn only_a_higher_version_counts_as_an_update() {
        let releases = parse_releases(LISTING).unwrap();
        let newest = &releases[0];
        assert!(newest.is_newer_than("2.3.0"));
        assert!(!newest.is_newer_than("2.3.1"));
        assert!(!newest.is_newer_than("3.0.0"));
    }

    #[test]
    fn a_listing_that_is_not_a_list_is_refused() {
        assert!(parse_releases("{}").is_err());
        assert!(parse_releases("not json").is_err());
        assert!(parse_releases("[]").unwrap().is_empty());
    }

    #[test]
    fn staged_and_retired_names_sit_beside_the_executable() {
        let exe = Path::new(r"C:\Apps\RustBlox.exe");
        assert_eq!(staged_path(exe), PathBuf::from(r"C:\Apps\RustBlox.exe.new"));
        assert_eq!(
            retired_path(exe),
            PathBuf::from(r"C:\Apps\RustBlox.exe.old")
        );
    }

    #[test]
    fn only_a_windows_program_is_accepted() {
        assert!(looks_like_a_program(b"MZ\x90\x00"));
        assert!(!looks_like_a_program(b"<!DOCTYPE html>"));
        assert!(!looks_like_a_program(b""));
    }

    #[test]
    fn swapping_puts_the_new_build_in_place_and_keeps_the_old_one() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("RustBlox.exe");
        let staged = staged_path(&exe);
        std::fs::write(&exe, b"old build").unwrap();
        std::fs::write(&staged, b"new build").unwrap();

        swap_in(&staged, &exe).unwrap();

        assert_eq!(std::fs::read(&exe).unwrap(), b"new build".to_vec());
        assert_eq!(
            std::fs::read(retired_path(&exe)).unwrap(),
            b"old build".to_vec()
        );
        assert!(!staged.exists());
    }

    #[test]
    fn swapping_without_a_download_leaves_the_executable_alone() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("RustBlox.exe");
        std::fs::write(&exe, b"old build").unwrap();

        assert!(swap_in(&staged_path(&exe), &exe).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"old build".to_vec());
    }

    #[test]
    fn clearing_removes_both_leftovers() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("RustBlox.exe");
        std::fs::write(&exe, b"live").unwrap();
        std::fs::write(retired_path(&exe), b"old").unwrap();
        std::fs::write(staged_path(&exe), b"half").unwrap();

        clear_retired(&exe);

        assert!(exe.is_file());
        assert!(!retired_path(&exe).exists());
        assert!(!staged_path(&exe).exists());
    }
}

#[cfg(test)]
mod live {
    use super::*;

    #[test]
    #[ignore = "reaches the GitHub API"]
    fn the_release_list_can_be_read() {
        let found = newest().expect("the release list should parse");
        match &found {
            Some(release) => println!(
                "newest is {} ({}) at {}",
                release.tag,
                crate::roblox::install::format_size(release.size),
                release.url
            ),
            None => println!("no published release carries a {ASSET} asset yet"),
        }
    }
}
