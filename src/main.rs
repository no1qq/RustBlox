#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod config;
mod error;
mod platform;
mod roblox;
mod selfupdate;
mod ui;
mod util;

use std::process::ExitCode;

use app::AppState;
use cli::{CommandKind, Invocation};
use config::{Paths, Store, WindowState};
use error::Result;

fn main() -> ExitCode {
    let invocation = cli::parse(std::env::args().skip(1));

    match &invocation.command_kind {
        CommandKind::Print(text) => {
            print_to_console(text);
            ExitCode::SUCCESS
        }
        CommandKind::Error(message) => {
            print_to_console(&format!("RustBlox: {message}\n"));
            ExitCode::FAILURE
        }
        _ => match run(invocation) {
            Ok(()) => ExitCode::SUCCESS,
            Err(err) => {
                log_error!("startup failed: {err}");
                report_fatal(&err);
                ExitCode::FAILURE
            }
        },
    }
}

fn run(invocation: Invocation) -> Result<()> {
    let paths = if invocation.portable {
        let root = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("rustblox-data")))
            .ok_or(error::Error::NoDataDir)?;
        Paths::rooted(root)
    } else {
        Paths::discover()?
    };

    util::fs::ensure_dir(paths.data_dir())?;
    util::fs::ensure_dir(paths.config_dir())?;
    util::log::attach(paths.log_file());
    log_info!("RustBlox {} starting", env!("CARGO_PKG_VERSION"));

    let store = Store::new(paths);
    let state = AppState::new(store, &invocation);
    util::log::set_file_logging(state.settings.advanced.keep_launch_logs);

    let window = state.persisted.window.sanitised();
    let command = invocation.command_kind.clone();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("RustBlox")
            .with_app_id("rustblox")
            .with_inner_size([window.width, window.height])
            .with_min_inner_size([WindowState::MIN_WIDTH, WindowState::MIN_HEIGHT])
            .with_maximized(window.maximized)
            .with_decorations(false)
            .with_resizable(true)
            .with_icon(ui::appicon::window_icon()),
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "RustBlox",
        options,
        Box::new(move |cc| Ok(Box::new(ui::RustBloxApp::new(cc, state, &command)))),
    )
    .map_err(|err| error::Error::invalid(format!("the window could not be created: {err}")))
}

fn print_to_console(text: &str) {
    #[cfg(windows)]
    platform::attach_parent_console();
    print!("{text}");
    use std::io::Write;
    let _ = std::io::stdout().flush();
}

fn report_fatal(err: &error::Error) {
    let mut body = err.to_string();
    if let Some(hint) = err.hint() {
        body.push_str("\n\n");
        body.push_str(hint);
    }

    rfd::MessageDialog::new()
        .set_title("RustBlox could not start")
        .set_description(&body)
        .set_level(rfd::MessageLevel::Error)
        .show();
}
