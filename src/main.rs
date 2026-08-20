#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod cli;
mod config;
mod error;
mod platform;
mod roblox;
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
            print_to_console(&format!("rustblox: {message}\n"));
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
            .with_icon(window_icon()),
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

fn window_icon() -> egui::IconData {
    const SIDE: usize = 64;
    let mut rgba = vec![0u8; SIDE * SIDE * 4];

    let accent = [255u8, 122, 69];
    let backdrop = [23u8, 26, 34];
    let radius = 13.0;

    for y in 0..SIDE {
        for x in 0..SIDE {
            let index = (y * SIDE + x) * 4;
            let inside = rounded_rect_coverage(x as f32, y as f32, SIDE as f32, radius);
            if inside <= 0.0 {
                continue;
            }

            let glyph = glyph_coverage(x as f32, y as f32, SIDE as f32);
            let base = if glyph > 0.5 { accent } else { backdrop };

            rgba[index] = base[0];
            rgba[index + 1] = base[1];
            rgba[index + 2] = base[2];
            rgba[index + 3] = (inside * 255.0) as u8;
        }
    }

    egui::IconData {
        rgba,
        width: SIDE as u32,
        height: SIDE as u32,
    }
}

fn rounded_rect_coverage(x: f32, y: f32, side: f32, radius: f32) -> f32 {
    let half = side / 2.0;
    let dx = (x - half + 0.5).abs() - (half - radius);
    let dy = (y - half + 0.5).abs() - (half - radius);
    let distance = if dx > 0.0 && dy > 0.0 {
        (dx * dx + dy * dy).sqrt() - radius
    } else {
        dx.max(dy) - radius
    };
    (0.5 - distance).clamp(0.0, 1.0)
}

fn glyph_coverage(x: f32, y: f32, side: f32) -> f32 {
    let nx = x / side;
    let ny = y / side;

    let apex_x = 0.5;
    let apex_y = 0.22;
    let base_y = 0.76;

    if ny < apex_y || ny > base_y {
        return 0.0;
    }

    let progress = (ny - apex_y) / (base_y - apex_y);
    let half_width = progress * 0.28;
    let outer = (nx - apex_x).abs() <= half_width;
    let inner = (nx - apex_x).abs() <= (half_width - 0.075).max(0.0) && ny > apex_y + 0.16;

    if outer && !inner {
        1.0
    } else {
        0.0
    }
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
