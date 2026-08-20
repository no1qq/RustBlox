mod about;
mod flags;
mod home;
mod installation;
mod settings;

use crate::app::AppState;

use super::{Page, UiState};

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    match ui_state.page {
        Page::Home => home::render(ui, state, ui_state),
        Page::Installation => installation::render(ui, state, ui_state),
        Page::Flags => flags::render(ui, state, ui_state),
        Page::Settings => settings::render(ui, state, ui_state),
        Page::About => about::render(ui, state, ui_state),
    }
}
