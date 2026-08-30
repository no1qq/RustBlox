mod about;
mod accounts;
mod flags;
mod game;
mod home;
mod installation;
mod mods;
mod settings;
mod shortcuts;

use crate::app::AppState;

use super::{Page, UiState};

pub fn render(ui: &mut egui::Ui, state: &mut AppState, ui_state: &mut UiState) {
    match ui_state.page {
        Page::Home => home::render(ui, state, ui_state),
        Page::Game => game::render(ui, state),
        Page::Mods => mods::render(ui, state),
        Page::Installation => installation::render(ui, state, ui_state),
        Page::FFlags => flags::render(ui, state, ui_state),
        Page::Shortcuts => shortcuts::render(ui, state),
        Page::Accounts => accounts::render(ui, state, ui_state),
        Page::Settings => settings::render(ui, state, ui_state),
        Page::About => about::render(ui, state, ui_state),
    }
}
