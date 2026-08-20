use std::io::Read;
use std::time::Duration;

use crate::error::{Error, Result};

pub const DEFAULT_CHANNEL: &str = "LIVE";
const CLIENT_SETTINGS: &str = "https://clientsettingscdn.roblox.com";
const MAX_MANIFEST_BYTES: usize = 512 * 1024;
const TIMEOUT: Duration = Duration::from_secs(30);

pub const MIRRORS: [&str; 4] = [
    "https://setup.rbxcdn.com",
    "https://setup-aws.rbxcdn.com",
    "https://setup-ak.rbxcdn.com",
    "https://s3.amazonaws.com/setup.roblox.com",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Deployment {
    pub channel: String,
    pub version: String,
    pub folder: String,
}

impl Deployment {
    pub fn package_url(&self, mirror: &str, package: &str) -> String {
        format!("{mirror}/{}-{package}", self.folder)
    }

    pub fn manifest_url(&self, mirror: &str) -> String {
        format!("{mirror}/{}-rbxPkgManifest.txt", self.folder)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    pub checksum: String,
    pub packed_size: u64,
    pub unpacked_size: u64,
}

impl Package {
    pub fn target(&self) -> Option<&'static str> {
        package_target(&self.name)
    }

    pub fn is_extractable(&self) -> bool {
        self.name.to_ascii_lowercase().ends_with(".zip") && self.target().is_some()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub packages: Vec<Package>,
}

impl Manifest {
    pub fn extractable(&self) -> impl Iterator<Item = &Package> {
        self.packages
            .iter()
            .filter(|package| package.is_extractable())
    }

    pub fn download_size(&self) -> u64 {
        self.extractable().map(|package| package.packed_size).sum()
    }

    pub fn install_size(&self) -> u64 {
        self.extractable()
            .map(|package| package.unpacked_size)
            .sum()
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut lines = text.lines().map(str::trim).filter(|line| !line.is_empty());

        match lines.next() {
            Some("v0") => {}
            Some(other) => {
                return Err(Error::invalid(format!(
                    "unrecognised package manifest format {other:?}"
                )))
            }
            None => return Err(Error::invalid("the package manifest was empty")),
        }

        let fields: Vec<&str> = lines.collect();
        if fields.len() % 4 != 0 {
            return Err(Error::invalid(
                "the package manifest ended part way through an entry",
            ));
        }

        let mut packages = Vec::with_capacity(fields.len() / 4);
        for entry in fields.chunks_exact(4) {
            let packed = entry[2].parse::<u64>().map_err(|_| {
                Error::invalid(format!("package {} has an unreadable size", entry[0]))
            })?;
            let unpacked = entry[3].parse::<u64>().map_err(|_| {
                Error::invalid(format!("package {} has an unreadable size", entry[0]))
            })?;

            packages.push(Package {
                name: entry[0].to_owned(),
                checksum: entry[1].to_ascii_lowercase(),
                packed_size: packed,
                unpacked_size: unpacked,
            });
        }

        if packages.is_empty() {
            return Err(Error::invalid("the package manifest listed no packages"));
        }

        Ok(Self { packages })
    }
}

pub const DELIBERATELY_SKIPPED: [&str; 1] = ["webview2runtimeinstaller.zip"];

pub fn is_deliberately_skipped(name: &str) -> bool {
    let lowered = name.to_ascii_lowercase();
    DELIBERATELY_SKIPPED.contains(&lowered.as_str())
}

pub fn package_target(name: &str) -> Option<&'static str> {
    let target = match name.to_ascii_lowercase().as_str() {
        "robloxapp.zip" | "webview2.zip" => "",
        "shaders.zip" => "shaders",
        "ssl.zip" => "ssl",

        "content-avatar.zip" => "content/avatar",
        "content-configs.zip" => "content/configs",
        "content-fonts.zip" => "content/fonts",
        "content-models.zip" => "content/models",
        "content-sky.zip" => "content/sky",
        "content-sounds.zip" => "content/sounds",
        "content-textures2.zip" => "content/textures",

        "content-platform-fonts.zip" => "PlatformContent/pc/fonts",
        "content-platform-dictionaries.zip" => "PlatformContent/pc/shared_compression_dictionaries",
        "content-terrain.zip" => "PlatformContent/pc/terrain",
        "content-textures3.zip" => "PlatformContent/pc/textures",

        "extracontent-luapackages.zip" => "ExtraContent/LuaPackages",
        "extracontent-models.zip" => "ExtraContent/models",
        "extracontent-places.zip" => "ExtraContent/places",
        "extracontent-textures.zip" => "ExtraContent/textures",
        "extracontent-translations.zip" => "ExtraContent/translations",

        _ => return None,
    };
    Some(target)
}

pub const APP_SETTINGS_BODY: &str = concat!(
    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
    "<Settings>\n",
    "\t<ContentFolder>content</ContentFolder>\n",
    "\t<BaseUrl>http://www.roblox.com</BaseUrl>\n",
    "</Settings>\n"
);

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(concat!("RustBlox/", env!("CARGO_PKG_VERSION")))
        .build()
        .into()
}

#[derive(serde::Deserialize)]
struct ClientVersion {
    version: String,
    #[serde(rename = "clientVersionUpload")]
    client_version_upload: String,
}

pub fn latest(channel: &str) -> Result<Deployment> {
    let channel = normalise_channel(channel);
    let url = if channel.eq_ignore_ascii_case(DEFAULT_CHANNEL) {
        format!("{CLIENT_SETTINGS}/v2/client-version/WindowsPlayer")
    } else {
        format!("{CLIENT_SETTINGS}/v2/client-version/WindowsPlayer/channel/{channel}")
    };

    let body = get_text(&url, 64 * 1024)?;
    let parsed: ClientVersion = serde_json::from_str(&body).map_err(|source| Error::Malformed {
        file: "client version response".into(),
        source,
    })?;

    if !parsed.client_version_upload.starts_with("version-") {
        return Err(Error::invalid(format!(
            "Roblox reported an unexpected version folder {:?}",
            parsed.client_version_upload
        )));
    }

    Ok(Deployment {
        channel,
        version: parsed.version,
        folder: parsed.client_version_upload,
    })
}

pub fn manifest(deployment: &Deployment) -> Result<Manifest> {
    let mut last = None;

    for mirror in MIRRORS {
        match get_text(&deployment.manifest_url(mirror), MAX_MANIFEST_BYTES) {
            Ok(text) => return Manifest::parse(&text),
            Err(err) => last = Some(err),
        }
    }

    Err(last.unwrap_or_else(|| Error::invalid("no download mirror could be reached")))
}

fn get_text(url: &str, limit: usize) -> Result<String> {
    let mut response = agent()
        .get(url)
        .call()
        .map_err(|err| Error::invalid(format!("{url} could not be reached: {err}")))?;

    let mut body = String::new();
    response
        .body_mut()
        .as_reader()
        .take(limit as u64)
        .read_to_string(&mut body)
        .map_err(|err| Error::io(format!("could not read the response from {url}"), err))?;

    Ok(body)
}

pub fn normalise_channel(channel: &str) -> String {
    let trimmed = channel.trim();
    if trimmed.is_empty() {
        return DEFAULT_CHANNEL.to_owned();
    }
    trimmed.to_owned()
}

pub fn is_valid_channel(channel: &str) -> bool {
    let trimmed = channel.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 64
        && trimmed
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "v0\n\
RobloxApp.zip\nf126998b7396c0f3786046bdec82ca44\n135172509\n173880875\n\
content-textures2.zip\ndcfe8c976632c8f4ef80ed2e1876269b\n5736616\n5984428\n\
RobloxPlayerInstaller.exe\n01efe1df5326f000751be197341731a0\n13578192\n13578192\n";

    #[test]
    fn parses_the_manifest_format() {
        let manifest = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(manifest.packages.len(), 3);

        let first = &manifest.packages[0];
        assert_eq!(first.name, "RobloxApp.zip");
        assert_eq!(first.checksum, "f126998b7396c0f3786046bdec82ca44");
        assert_eq!(first.packed_size, 135172509);
        assert_eq!(first.unpacked_size, 173880875);
    }

    #[test]
    fn skips_packages_that_are_not_extractable() {
        let manifest = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(manifest.extractable().count(), 2);
        assert_eq!(manifest.download_size(), 135172509 + 5736616);
        assert_eq!(manifest.install_size(), 173880875 + 5984428);
    }

    #[test]
    fn rejects_broken_manifests() {
        assert!(Manifest::parse("").is_err());
        assert!(Manifest::parse("v9\nRobloxApp.zip\n").is_err());
        assert!(Manifest::parse("v0\nRobloxApp.zip\nabc\n1\n").is_err());
        assert!(Manifest::parse("v0\nRobloxApp.zip\nabc\nnotanumber\n2\n").is_err());
        assert!(Manifest::parse("v0\n").is_err());
    }

    #[test]
    fn maps_every_package_to_the_folder_roblox_uses() {
        assert_eq!(package_target("RobloxApp.zip"), Some(""));
        assert_eq!(package_target("shaders.zip"), Some("shaders"));
        assert_eq!(
            package_target("content-textures2.zip"),
            Some("content/textures")
        );
        assert_eq!(
            package_target("content-textures3.zip"),
            Some("PlatformContent/pc/textures")
        );
        assert_eq!(
            package_target("extracontent-luapackages.zip"),
            Some("ExtraContent/LuaPackages")
        );
        assert_eq!(package_target("RobloxPlayerInstaller.exe"), None);
        assert_eq!(package_target("something-new.zip"), None);
    }

    #[test]
    fn package_names_are_matched_case_insensitively() {
        assert_eq!(package_target("ROBLOXAPP.ZIP"), Some(""));
        assert_eq!(package_target("Content-Fonts.zip"), Some("content/fonts"));
    }

    #[test]
    fn builds_mirror_urls() {
        let deployment = Deployment {
            channel: "LIVE".into(),
            version: "0.735.0.7351131".into(),
            folder: "version-ce0bcd0fbd484804".into(),
        };
        assert_eq!(
            deployment.manifest_url(MIRRORS[0]),
            "https://setup.rbxcdn.com/version-ce0bcd0fbd484804-rbxPkgManifest.txt"
        );
        assert_eq!(
            deployment.package_url(MIRRORS[0], "shaders.zip"),
            "https://setup.rbxcdn.com/version-ce0bcd0fbd484804-shaders.zip"
        );
    }

    #[test]
    fn validates_channel_names() {
        assert_eq!(normalise_channel("  "), "LIVE");
        assert_eq!(normalise_channel(" ZCanary "), "ZCanary");
        assert!(is_valid_channel("LIVE"));
        assert!(is_valid_channel("ZNext"));
        assert!(!is_valid_channel(""));
        assert!(!is_valid_channel("has space"));
        assert!(!is_valid_channel("../escape"));
    }
}

#[cfg(test)]
mod skip_tests {
    use super::*;

    #[test]
    fn the_webview_runtime_installer_is_skipped_on_purpose() {
        assert!(is_deliberately_skipped("WebView2RuntimeInstaller.zip"));
        assert!(is_deliberately_skipped("webview2runtimeinstaller.zip"));
        assert!(package_target("WebView2RuntimeInstaller.zip").is_none());
    }

    #[test]
    fn the_webview_loader_itself_is_still_installed() {
        assert!(!is_deliberately_skipped("WebView2.zip"));
        assert_eq!(package_target("WebView2.zip"), Some(""));
    }

    #[test]
    fn an_unknown_package_is_not_treated_as_skipped() {
        assert!(!is_deliberately_skipped("content-brandnew.zip"));
    }
}
