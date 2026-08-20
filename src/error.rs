use std::path::{Path, PathBuf};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },

    #[error("could not resolve a per-user application data directory")]
    NoDataDir,

    #[error("{file} is not valid JSON: {source}")]
    Malformed {
        file: String,
        #[source]
        source: serde_json::Error,
    },

    #[error("no Roblox installation could be found on this system")]
    RobloxNotFound,

    #[error("the Roblox player executable is missing from {}", .0.display())]
    PlayerMissing(PathBuf),

    #[error("Roblox could not be started: {0}")]
    LaunchFailed(String),

    #[error("{0}")]
    Registry(String),

    #[cfg(not(windows))]
    #[error("this feature is only available on Windows")]
    UnsupportedPlatform,

    #[error("{0}")]
    Invalid(String),
}

impl Error {
    pub fn io(context: impl Into<String>, source: std::io::Error) -> Self {
        Error::Io {
            context: context.into(),
            source,
        }
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Error::Invalid(message.into())
    }

    pub fn registry(message: impl Into<String>) -> Self {
        Error::Registry(message.into())
    }

    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Error::RobloxNotFound => Some(
                "Install Roblox from roblox.com, or point RustBlox at an existing installation in Settings.",
            ),
            Error::PlayerMissing(_) => Some(
                "Roblox may have updated or been moved. Run a rescan from the Installation page.",
            ),
            Error::NoDataDir => {
                Some("Check that your Windows user profile and APPDATA variables are intact.")
            }
            Error::Malformed { .. } => {
                Some("The damaged file was set aside and defaults were restored.")
            }
            Error::LaunchFailed(_) => {
                Some("Close any running Roblox windows and try again.")
            }
            _ => None,
        }
    }
}

pub trait Context<T> {
    fn ctx_path(self, action: &str, path: &Path) -> Result<T>;
}

impl<T> Context<T> for std::result::Result<T, std::io::Error> {
    fn ctx_path(self, action: &str, path: &Path) -> Result<T> {
        self.map_err(|source| Error::io(format!("{action} {}", path.display()), source))
    }
}
