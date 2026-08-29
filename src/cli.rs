use std::path::PathBuf;

use crate::roblox::uri;

pub const HELP: &str = "\
RustBlox - a desktop client and launcher for Roblox

USAGE:
    RustBlox [OPTIONS]

OPTIONS:
    --forward <uri>   Hand a roblox: or roblox-player: link to the Roblox client
    --launch          Start Roblox using the configured startup target
    --settings        Open the full window on the Settings page
    --reset           Start with default settings, keeping the old file as a backup
    --portable        Keep configuration next to the executable instead of AppData
    -v, --version     Print the version and exit
    -h, --help        Print this help and exit

A bare roblox: or roblox-player: link is treated the same as --forward.
";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Invocation {
    pub command_kind: CommandKind,
    pub reset: bool,
    pub portable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum CommandKind {
    #[default]
    Window,
    WindowOnSettings,
    LaunchNow,
    TheWatcher {
        pid: u32,
        install_dir: PathBuf,
    },
    Forward(String),
    Print(String),
    Error(String),
}

pub fn parse<I, S>(args: I) -> Invocation
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut invocation = Invocation::default();
    let mut iter = args
        .into_iter()
        .map(|arg| arg.as_ref().to_owned())
        .peekable();

    while let Some(arg) = iter.next() {
        let lowered = arg.to_ascii_lowercase();
        match lowered.as_str() {
            "-h" | "--help" | "/?" => {
                invocation.command_kind = CommandKind::Print(HELP.to_owned());
                return invocation;
            }
            "-v" | "--version" => {
                invocation.command_kind =
                    CommandKind::Print(format!("RustBlox {}\n", env!("CARGO_PKG_VERSION")));
                return invocation;
            }
            "--reset" => invocation.reset = true,
            "--portable" => invocation.portable = true,
            "--settings" => invocation.command_kind = CommandKind::WindowOnSettings,
            "--launch" => invocation.command_kind = CommandKind::LaunchNow,
            "--thewatcher" => {
                let pid_str = iter.next();
                let dir_str = iter.next();
                if let (Some(pid_val), Some(dir_val)) = (pid_str, dir_str) {
                    if let Ok(pid) = pid_val.parse::<u32>() {
                        invocation.command_kind = CommandKind::TheWatcher {
                            pid,
                            install_dir: PathBuf::from(dir_val),
                        };
                        return invocation;
                    }
                }
                invocation.command_kind =
                    CommandKind::Error("--thewatcher requires a PID and install directory".into());
                return invocation;
            }
            "--forward" | "-player" | "--player" => match iter.next() {
                Some(value) => match uri::validate(&value) {
                    Ok(clean) => invocation.command_kind = CommandKind::Forward(clean),
                    Err(err) => {
                        invocation.command_kind = CommandKind::Error(err.to_string());
                        return invocation;
                    }
                },
                None => {
                    invocation.command_kind =
                        CommandKind::Error("--forward needs a launch link".into());
                    return invocation;
                }
            },
            other => {
                if uri::is_launch_uri(other) {
                    match uri::validate(&arg) {
                        Ok(clean) => invocation.command_kind = CommandKind::Forward(clean),
                        Err(err) => {
                            invocation.command_kind = CommandKind::Error(err.to_string());
                            return invocation;
                        }
                    }
                } else if other.starts_with('-') {
                    invocation.command_kind =
                        CommandKind::Error(format!("unknown option {arg}\n\n{HELP}"));
                    return invocation;
                }
            }
        }
    }

    invocation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_to_opening_the_window() {
        let parsed = parse(Vec::<String>::new());
        assert_eq!(parsed.command_kind, CommandKind::Window);
        assert!(!parsed.reset);
    }

    #[test]
    fn recognises_a_bare_launch_link() {
        let parsed = parse(["roblox-player:1+launchmode:app"]);
        assert_eq!(
            parsed.command_kind,
            CommandKind::Forward("roblox-player:1+launchmode:app".into())
        );
    }

    #[test]
    fn forward_requires_a_value() {
        let parsed = parse(["--forward"]);
        assert!(matches!(parsed.command_kind, CommandKind::Error(_)));
    }

    #[test]
    fn rejects_links_from_other_schemes() {
        let parsed = parse(["--forward", "https://example.com"]);
        assert!(matches!(parsed.command_kind, CommandKind::Error(_)));
    }

    #[test]
    fn collects_modifier_flags() {
        let parsed = parse(["--portable", "--reset", "--settings"]);
        assert!(parsed.portable);
        assert!(parsed.reset);
        assert_eq!(parsed.command_kind, CommandKind::WindowOnSettings);
    }

    #[test]
    fn unknown_options_are_reported() {
        let parsed = parse(["--wat"]);
        assert!(matches!(parsed.command_kind, CommandKind::Error(_)));
    }

    #[test]
    fn parses_thewatcher_arguments() {
        let parsed = parse(["--thewatcher", "1234", "C:/Roblox/Versions"]);
        assert_eq!(
            parsed.command_kind,
            CommandKind::TheWatcher {
                pid: 1234,
                install_dir: PathBuf::from("C:/Roblox/Versions"),
            }
        );
    }
}
