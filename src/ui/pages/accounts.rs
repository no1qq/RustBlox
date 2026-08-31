use egui::{Align, Layout, RichText, StrokeKind, Ui, Vec2};

use crate::app::AppState;
use crate::roblox::account::{self, QuickSignInPollResult};
use crate::roblox::launch::LaunchTarget;
use crate::ui::icons::Icon;
use crate::ui::theme::{self, Theme};
use crate::ui::widgets;
use crate::ui::{AccountsTab, UiState};

pub fn render(ui: &mut Ui, state: &mut AppState, ui_state: &mut UiState) {
    let theme = Theme::get(ui.ctx());

    widgets::page_header(
        ui,
        "Account Manager",
        "Manage multiple Roblox accounts, switch profiles, and join friends in-game.",
        |ui| {
            subtab_selector(ui, &theme, ui_state);
        },
    );

    ui.add_space(theme.metrics.gap_lg);

    match ui_state.accounts_tab {
        AccountsTab::Accounts => render_accounts_tab(ui, &theme, state, ui_state),
        AccountsTab::Friends => render_friends_tab(ui, &theme, state, ui_state),
    }

    quick_sign_in_modal(ui.ctx(), &theme, state, ui_state);
    manual_account_modal(ui.ctx(), &theme, state, ui_state);
}

fn subtab_selector(ui: &mut Ui, theme: &Theme, ui_state: &mut UiState) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.metrics.gap_xs;
        for tab in AccountsTab::ALL {
            let active = ui_state.accounts_tab == tab;
            let tone = if active {
                widgets::Tone::Primary
            } else {
                widgets::Tone::Ghost
            };
            if widgets::Button::new(tab.label())
                .icon(tab.icon())
                .tone(tone)
                .size(widgets::Size::Small)
                .show(ui)
                .clicked()
            {
                ui_state.accounts_tab = tab;
            }
        }
    });
}

static AVATAR_IMAGE_CACHE: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<u64, Option<Vec<u8>>>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

static AVATAR_IN_FLIGHT: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<u64>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

fn get_or_fetch_avatar_texture(ctx: &egui::Context, user_id: u64) -> Option<egui::TextureHandle> {
    let key = egui::Id::new(("user-avatar-texture", user_id));
    if let Some(handle) = ctx.data(|d| d.get_temp::<egui::TextureHandle>(key)) {
        return Some(handle);
    }

    let cached_bytes = {
        let guard = AVATAR_IMAGE_CACHE.lock().ok()?;
        guard.get(&user_id).cloned()
    };

    if let Some(maybe_bytes) = cached_bytes {
        if let Some(bytes) = maybe_bytes {
            if let Some(color_img) = account::decode_avatar_png(&bytes) {
                let handle = ctx.load_texture(
                    format!("avatar-{user_id}"),
                    color_img,
                    egui::TextureOptions::LINEAR,
                );
                ctx.data_mut(|d| d.insert_temp(key, handle.clone()));
                return Some(handle);
            }
        }
        return None;
    }

    let should_fetch = {
        if let Ok(mut in_flight) = AVATAR_IN_FLIGHT.lock() {
            in_flight.insert(user_id)
        } else {
            false
        }
    };

    if should_fetch {
        let ctx_clone = ctx.clone();
        std::thread::Builder::new()
            .name(format!("avatar-fetch-{user_id}"))
            .spawn(move || {
                let bytes = account::fetch_avatar_image_bytes(user_id).ok();
                if let Ok(mut cache) = AVATAR_IMAGE_CACHE.lock() {
                    cache.insert(user_id, bytes);
                }
                if let Ok(mut in_flight) = AVATAR_IN_FLIGHT.lock() {
                    in_flight.remove(&user_id);
                }
                ctx_clone.request_repaint();
            })
            .ok();
    }

    None
}

fn draw_avatar(
    ui: &mut Ui,
    rect: egui::Rect,
    user_id: Option<u64>,
    name: &str,
    is_active: bool,
    theme: &Theme,
) {
    let radius = rect.width() / 2.0;
    let bg_color = if is_active {
        theme.palette.accent.gamma_multiply(0.2)
    } else {
        theme.palette.surface_hover
    };
    ui.painter().circle_filled(rect.center(), radius, bg_color);
    ui.painter().circle_stroke(
        rect.center(),
        radius,
        egui::Stroke::new(
            if is_active { 1.5 } else { 1.0 },
            if is_active {
                theme.palette.accent
            } else {
                theme.palette.border
            },
        ),
    );

    let mut rendered_texture = false;
    if let Some(id) = user_id {
        if let Some(tex) = get_or_fetch_avatar_texture(ui.ctx(), id) {
            ui.painter().add(
                egui::epaint::RectShape::filled(
                    rect.shrink(1.0),
                    theme.radius_lg(),
                    egui::Color32::WHITE,
                )
                .with_texture(
                    tex.id(),
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                ),
            );
            rendered_texture = true;
        }
    }

    if !rendered_texture {
        let initial = name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            &initial,
            egui::FontId::proportional(radius * 0.85),
            if is_active {
                theme.palette.accent
            } else {
                theme.palette.text
            },
        );
    }
}

fn render_accounts_tab(ui: &mut Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    current_account_card(ui, theme, state);
    ui.add_space(theme.metrics.gap_lg);
    manage_accounts_card(ui, theme, state, ui_state);
}

fn current_account_card(ui: &mut Ui, theme: &Theme, state: &mut AppState) {
    let active_account = state
        .active_account_id
        .and_then(|id| state.accounts.iter().find(|acc| acc.id == id).cloned());

    widgets::section(
        ui,
        "Current Account",
        Some("The account currently active in Roblox Player and Windows Registry."),
        |ui| {
            widgets::nested(ui, |ui| {
                ui.horizontal(|ui| {
                    let avatar_size = Vec2::splat(44.0);
                    let (avatar_rect, _) =
                        ui.allocate_exact_size(avatar_size, egui::Sense::hover());
                    draw_avatar(
                        ui,
                        avatar_rect,
                        active_account.as_ref().map(|a| a.id),
                        active_account
                            .as_ref()
                            .map(|a| a.display_name.as_str())
                            .unwrap_or("?"),
                        active_account.is_some(),
                        theme,
                    );

                    ui.add_space(theme.metrics.gap_sm);

                    ui.vertical(|ui| {
                        if let Some(acc) = &active_account {
                            ui.horizontal(|ui| {
                                ui.label(
                                    RichText::new(&acc.display_name)
                                        .font(theme::strong(theme::size::TITLE))
                                        .color(theme.palette.text),
                                );
                                ui.label(
                                    RichText::new(format!("@{}", acc.username))
                                        .font(theme::text_style(theme::size::BODY))
                                        .color(theme.palette.text_muted),
                                );
                                widgets::badge(ui, "Active", widgets::feedback::Tone::Success);
                            });
                            ui.label(
                                RichText::new(format!("User ID: {}", acc.id))
                                    .font(theme::text_style(theme::size::SMALL))
                                    .color(theme.palette.text_muted),
                            );
                        } else {
                            ui.label(
                                RichText::new("Not Logged In")
                                    .font(theme::strong(theme::size::TITLE))
                                    .color(theme.palette.text),
                            );
                            ui.label(
                                RichText::new(
                                    "Sign in to sync your Roblox profile and switch accounts.",
                                )
                                .font(theme::text_style(theme::size::SMALL))
                                .color(theme.palette.text_muted),
                            );
                        }
                    });

                    if active_account.is_some() {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widgets::Button::new("Sign Out")
                                .tone(widgets::Tone::Neutral)
                                .size(widgets::Size::Small)
                                .show(ui)
                                .clicked()
                            {
                                state.active_account_id = None;
                                let _ = account::apply_account_session("");
                                state.toasts.info("Signed out from active account");
                            }
                        });
                    }
                });
            });
        },
    );
}

fn manage_accounts_card(ui: &mut Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    widgets::section(
        ui,
        "Manage Accounts",
        Some("Select an account to switch sessions or add new accounts."),
        |ui| {
            if state.accounts.is_empty() {
                widgets::nested(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme.metrics.gap_lg);
                        let (icon_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(40.0), egui::Sense::hover());
                        crate::ui::icons::draw(
                            ui.painter(),
                            Icon::Users,
                            icon_rect,
                            theme.palette.text_muted,
                            1.5,
                        );
                        ui.add_space(theme.metrics.gap_sm);
                        ui.label(
                            RichText::new("No accounts saved yet")
                                .font(theme::strong(theme::size::BODY))
                                .color(theme.palette.text),
                        );
                        ui.label(
                            RichText::new(
                                "Add an account using Quick Sign-In code, Browser login, or your cookie.",
                            )
                            .font(theme::text_style(theme::size::SMALL))
                            .color(theme.palette.text_muted),
                        );
                        ui.add_space(theme.metrics.gap_lg);
                    });
                });
            } else {
                let mut switch_to = None;
                let mut remove_id = None;

                for acc in &state.accounts {
                    let is_active = state.active_account_id == Some(acc.id);
                    let is_selected = ui_state.selected_account_id == Some(acc.id);

                    let fill = if is_selected {
                        theme.palette.surface_hover
                    } else {
                        theme.palette.surface
                    };

                    let response = egui::Frame::new()
                        .fill(fill)
                        .stroke(egui::Stroke::new(
                            if is_selected { 1.5 } else { 1.0 },
                            if is_selected {
                                theme.palette.accent
                            } else {
                                theme.palette.border
                            },
                        ))
                        .corner_radius(theme.radius_md())
                        .inner_margin(egui::Margin::same(12))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let (avatar_rect, _) =
                                    ui.allocate_exact_size(Vec2::splat(36.0), egui::Sense::hover());
                                draw_avatar(
                                    ui,
                                    avatar_rect,
                                    Some(acc.id),
                                    &acc.display_name,
                                    is_active,
                                    theme,
                                );

                                ui.add_space(theme.metrics.gap_xs);

                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            RichText::new(&acc.display_name)
                                                .font(theme::strong(theme::size::BODY))
                                                .color(theme.palette.text),
                                        );
                                        ui.label(
                                            RichText::new(format!("@{}", acc.username))
                                                .font(theme::text_style(theme::size::SMALL))
                                                .color(theme.palette.text_muted),
                                        );
                                        if is_active {
                                            widgets::badge(
                                                ui,
                                                "Active",
                                                widgets::feedback::Tone::Success,
                                            );
                                        }
                                    });
                                    ui.label(
                                        RichText::new(format!(
                                            "ID: {} • Added: {}",
                                            acc.id, acc.created_at
                                        ))
                                        .font(theme::text_style(theme::size::SMALL))
                                        .color(theme.palette.text_muted),
                                    );
                                });

                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if widgets::Button::new("Remove")
                                        .tone(widgets::Tone::Danger)
                                        .size(widgets::Size::Small)
                                        .show(ui)
                                        .clicked()
                                    {
                                        remove_id = Some(acc.id);
                                    }
                                    if !is_active
                                        && widgets::Button::new("Switch")
                                            .tone(widgets::Tone::Primary)
                                            .size(widgets::Size::Small)
                                            .show(ui)
                                            .clicked()
                                    {
                                        switch_to = Some(acc.id);
                                    }
                                });
                            });
                        });

                    if response.response.interact(egui::Sense::click()).clicked() {
                        ui_state.selected_account_id = Some(acc.id);
                    }

                    ui.add_space(theme.metrics.gap_xs);
                }

                if let Some(id) = remove_id {
                    state.remove_account(id);
                    if ui_state.selected_account_id == Some(id) {
                        ui_state.selected_account_id = None;
                    }
                }
                if let Some(id) = switch_to {
                    state.switch_account(id);
                }
            }

            ui.add_space(theme.metrics.gap_md);

            ui.horizontal(|ui| {
                let has_selected = ui_state.selected_account_id.is_some();
                let is_selected_active = ui_state.selected_account_id == state.active_account_id;

                if widgets::Button::primary("Switch to Selected")
                    .enabled(has_selected && !is_selected_active)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    if let Some(id) = ui_state.selected_account_id {
                        state.switch_account(id);
                    }
                }

                ui.add_space(theme.metrics.gap_sm);

                if widgets::Button::new("Quick Sign-In (Code)")
                    .icon(Icon::External)
                    .tone(widgets::Tone::Neutral)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    ui_state.show_quick_sign_in_dialog = true;
                    match account::start_quick_sign_in() {
                        Ok(session) => {
                            ui_state.quick_sign_in_session = Some(session);
                            ui_state.quick_sign_in_status = "Ready - Enter code on Roblox".into();
                        }
                        Err(err) => {
                            ui_state.quick_sign_in_session = None;
                            ui_state.quick_sign_in_status = format!("Error: {err}");
                        }
                    }
                }

                if widgets::Button::new("Add Account")
                    .icon(Icon::Plus)
                    .tone(widgets::Tone::Neutral)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    ui_state.account_cookie_input.clear();
                    ui_state.account_error = None;
                    ui_state.show_manual_account_dialog = true;
                }
            });
        },
    );
}

fn render_friends_tab(ui: &mut Ui, theme: &Theme, state: &mut AppState, ui_state: &mut UiState) {
    let active_account = state
        .active_account_id
        .and_then(|id| state.accounts.iter().find(|acc| acc.id == id));

    let Some(active) = active_account else {
        widgets::section(
            ui,
            "Friends",
            Some("View friends list and join current games directly."),
            |ui| {
                widgets::nested(ui, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(theme.metrics.gap_lg);
                        ui.label(
                            RichText::new("No active Roblox account selected")
                                .font(theme::strong(theme::size::BODY))
                                .color(theme.palette.text),
                        );
                        ui.label(
                            RichText::new(
                                "Please add and activate an account in the Accounts tab.",
                            )
                            .font(theme::text_style(theme::size::SMALL))
                            .color(theme.palette.text_muted),
                        );
                        ui.add_space(theme.metrics.gap_lg);
                    });
                });
            },
        );
        return;
    };

    let user_id = active.id;
    let cookie = active.cookie.clone();

    if ui_state.friends_loaded_for != Some(user_id) && !ui_state.friends_loading {
        ui_state.friends_loading = true;
        match account::fetch_friends(user_id, &cookie) {
            Ok(friends) => {
                ui_state.friends_cache = friends;
                ui_state.friends_loaded_for = Some(user_id);
            }
            Err(err) => {
                state
                    .toasts
                    .error("Could not load friends", Some(err.to_string()));
                ui_state.friends_loaded_for = Some(user_id);
            }
        }
        ui_state.friends_loading = false;
    }

    widgets::section(
        ui,
        &format!("Friends for @{}", active.username),
        Some("Friends online and in-game. Click Join Game to launch directly into their place."),
        |ui| {
            ui.horizontal(|ui| {
                if widgets::Button::new("Refresh Friends")
                    .icon(Icon::Refresh)
                    .tone(widgets::Tone::Neutral)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    ui_state.friends_loaded_for = None;
                }
                ui.label(
                    RichText::new(format!("{} friends total", ui_state.friends_cache.len()))
                        .font(theme::text_style(theme::size::SMALL))
                        .color(theme.palette.text_muted),
                );
            });

            ui.add_space(theme.metrics.gap_md);

            if ui_state.friends_cache.is_empty() {
                widgets::nested(ui, |ui| {
                    ui.label(
                        RichText::new("No friends found or friends list is empty.")
                            .font(theme::text_style(theme::size::SMALL))
                            .color(theme.palette.text_muted),
                    );
                });
            } else {
                for friend in &ui_state.friends_cache {
                    widgets::nested(ui, |ui| {
                        ui.horizontal(|ui| {
                            let avatar_size = Vec2::splat(38.0);
                            let (avatar_rect, _) =
                                ui.allocate_exact_size(avatar_size, egui::Sense::hover());

                            let is_ingame = friend.presence_type == 2;
                            let is_online = friend.presence_type > 0;

                            draw_avatar(
                                ui,
                                avatar_rect,
                                Some(friend.id),
                                &friend.display_name,
                                is_ingame,
                                theme,
                            );

                            let status_dot_pos = avatar_rect.right_bottom() + Vec2::new(-4.0, -4.0);
                            let status_color = if is_ingame {
                                egui::Color32::from_rgb(34, 197, 94)
                            } else if is_online {
                                theme.palette.info
                            } else {
                                theme.palette.text_muted.gamma_multiply(0.4)
                            };
                            ui.painter()
                                .circle_filled(status_dot_pos, 5.0, theme.palette.surface);
                            ui.painter()
                                .circle_filled(status_dot_pos, 4.0, status_color);

                            ui.add_space(theme.metrics.gap_sm);

                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(&friend.display_name)
                                            .font(theme::strong(theme::size::BODY))
                                            .color(theme.palette.text),
                                    );
                                    if !friend.username.is_empty() {
                                        ui.label(
                                            RichText::new(format!("@{}", friend.username))
                                                .font(theme::text_style(theme::size::SMALL))
                                                .color(theme.palette.text_muted),
                                        );
                                    }
                                });

                                let status_text = match friend.presence_type {
                                    2 => {
                                        if let Some(loc) = &friend.last_location {
                                            format!("Playing: {loc}")
                                        } else {
                                            "In Game".to_string()
                                        }
                                    }
                                    1 => "Online".to_string(),
                                    3 => "In Roblox Studio".to_string(),
                                    _ => "Offline".to_string(),
                                };
                                ui.label(
                                    RichText::new(status_text)
                                        .font(theme::text_style(theme::size::SMALL))
                                        .color(if is_ingame {
                                            theme.palette.accent
                                        } else if is_online {
                                            theme.palette.info
                                        } else {
                                            theme.palette.text_muted
                                        }),
                                );
                            });

                            if let Some(place_id) = friend.place_id {
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    if widgets::Button::new("Join Game")
                                        .icon(Icon::Play)
                                        .tone(widgets::Tone::Primary)
                                        .size(widgets::Size::Small)
                                        .show(ui)
                                        .clicked()
                                    {
                                        state.start_launch_flow(LaunchTarget::Place {
                                            place_id,
                                            label: friend.last_location.clone(),
                                        });
                                    }
                                });
                            }
                        });
                    });
                    ui.add_space(theme.metrics.gap_xs);
                }
            }
        },
    );
}

fn quick_sign_in_modal(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut AppState,
    ui_state: &mut UiState,
) {
    if !ui_state.show_quick_sign_in_dialog {
        return;
    }

    let palette = theme.palette;
    let mut close = false;
    let mut poll_check = false;

    let now = std::time::Instant::now();
    if ui_state.quick_sign_in_session.is_some() {
        let should_poll = match ui_state.quick_sign_in_last_poll {
            Some(last) => now.duration_since(last) >= std::time::Duration::from_millis(1500),
            None => true,
        };
        if should_poll {
            ui_state.quick_sign_in_last_poll = Some(now);
            poll_check = true;
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(1500));
    }

    let response = egui::Modal::new(egui::Id::new("quick-sign-in-modal"))
        .backdrop_color(palette.scrim)
        .frame(
            egui::Frame::new()
                .fill(palette.surface)
                .stroke(egui::Stroke::new(1.0, palette.border))
                .corner_radius(theme.radius_lg())
                .inner_margin(egui::Margin::same(24)),
        )
        .show(ctx, |ui| {
            ui.set_width(480.0);

            ui.horizontal(|ui| {
                let (icon_rect, _) =
                    ui.allocate_exact_size(Vec2::splat(22.0), egui::Sense::hover());
                crate::ui::icons::draw(
                    ui.painter(),
                    Icon::External,
                    icon_rect,
                    palette.accent,
                    1.8,
                );
                ui.add_space(theme.metrics.gap_xs);
                ui.label(
                    RichText::new("Quick Sign In")
                        .font(theme::strong(theme::size::TITLE))
                        .color(palette.text),
                );
            });

            ui.add_space(theme.metrics.gap_sm);

            ui.label(
                RichText::new("Enter this code on Roblox Quick Sign-In or in the Roblox App:")
                    .font(theme::text_style(theme::size::BODY))
                    .color(palette.text_muted),
            );

            if let Some(session) = &ui_state.quick_sign_in_session {
                ui.add_space(theme.metrics.gap_md);

                let code_rect = ui
                    .allocate_exact_size(
                        Vec2::new(ui.available_width(), 56.0),
                        egui::Sense::hover(),
                    )
                    .0;
                ui.painter()
                    .rect_filled(code_rect, theme.radius_md(), palette.surface_hover);
                ui.painter().rect_stroke(
                    code_rect,
                    theme.radius_md(),
                    egui::Stroke::new(1.0, palette.border),
                    StrokeKind::Inside,
                );

                ui.painter().text(
                    code_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    &session.code,
                    egui::FontId::proportional(26.0),
                    palette.text,
                );

                ui.add_space(theme.metrics.gap_sm);

                widgets::nested(ui, |ui| {
                    ui.horizontal(|ui| {
                        let (info_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(16.0), egui::Sense::hover());
                        crate::ui::icons::draw(
                            ui.painter(),
                            Icon::Info,
                            info_rect,
                            palette.info,
                            1.5,
                        );
                        ui.add_space(theme.metrics.gap_xs);
                        ui.label(
                            RichText::new(&ui_state.quick_sign_in_status)
                                .font(theme::text_style(theme::size::SMALL))
                                .color(palette.text_muted),
                        );
                    });
                });

                ui.add_space(theme.metrics.gap_lg);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;

                    if widgets::Button::primary("Copy Code")
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        ui.ctx().copy_text(session.code.clone());
                        state.toasts.success("Copied code to clipboard");
                    }

                    if widgets::Button::new("Open Quick Login")
                        .icon(Icon::External)
                        .tone(widgets::Tone::Neutral)
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        state.open_url("https://www.roblox.com/crossdevicelogin/ConfirmCode");
                    }

                    if widgets::Button::new("Check Status")
                        .icon(Icon::Refresh)
                        .tone(widgets::Tone::Neutral)
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        poll_check = true;
                    }

                    if widgets::Button::new("Close")
                        .tone(widgets::Tone::Neutral)
                        .size(widgets::Size::Small)
                        .show(ui)
                        .clicked()
                    {
                        close = true;
                    }
                });
            } else {
                ui.add_space(theme.metrics.gap_md);
                ui.label(
                    RichText::new(&ui_state.quick_sign_in_status)
                        .font(theme::text_style(theme::size::BODY))
                        .color(palette.danger),
                );
                ui.add_space(theme.metrics.gap_md);
                if widgets::Button::new("Close")
                    .tone(widgets::Tone::Neutral)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    close = true;
                }
            }
        });

    if poll_check {
        if let Some(session) = &ui_state.quick_sign_in_session {
            match account::poll_quick_sign_in(session) {
                Ok(QuickSignInPollResult::Approved(cookie)) => {
                    if !cookie.is_empty() {
                        state.add_account(&cookie);
                        ui_state.show_quick_sign_in_dialog = false;
                        ui_state.quick_sign_in_session = None;
                        state
                            .toasts
                            .success("Successfully authenticated with Quick Sign-In!");
                    } else {
                        ui_state.quick_sign_in_status = "Approved! Finalizing login...".into();
                    }
                }
                Ok(QuickSignInPollResult::Pending(msg)) => {
                    ui_state.quick_sign_in_status = msg;
                }
                Ok(QuickSignInPollResult::Denied) => {
                    ui_state.quick_sign_in_status = "Login was denied on the device.".into();
                }
                Ok(QuickSignInPollResult::Expired) => {
                    ui_state.quick_sign_in_status =
                        "Code expired. Please request a new code.".into();
                }
                Ok(QuickSignInPollResult::Error(err)) => {
                    ui_state.quick_sign_in_status = format!("Poll error: {err}");
                }
                Err(err) => {
                    ui_state.quick_sign_in_status = format!("Failed to poll: {err}");
                }
            }
        }
    }

    if response.should_close() || close {
        ui_state.show_quick_sign_in_dialog = false;
        ui_state.quick_sign_in_session = None;
        ui_state.quick_sign_in_last_poll = None;
    }
}

fn manual_account_modal(
    ctx: &egui::Context,
    theme: &Theme,
    state: &mut AppState,
    ui_state: &mut UiState,
) {
    if !ui_state.show_manual_account_dialog {
        return;
    }

    let palette = theme.palette;
    let mut close = false;
    let mut save_cookie = None;

    let response = egui::Modal::new(egui::Id::new("manual-account-modal"))
        .backdrop_color(palette.scrim)
        .frame(
            egui::Frame::new()
                .fill(palette.surface)
                .stroke(egui::Stroke::new(1.0, palette.border))
                .corner_radius(theme.radius_lg())
                .inner_margin(egui::Margin::same(24)),
        )
        .show(ctx, |ui| {
            ui.set_width(460.0);

            ui.label(
                RichText::new("Add Roblox Account")
                    .font(theme::strong(theme::size::TITLE))
                    .color(palette.text),
            );
            ui.add_space(theme.metrics.gap_xs);
            ui.label(
                RichText::new(
                    "Paste your .ROBLOSECURITY session cookie or open Roblox in your browser.",
                )
                .font(theme::text_style(theme::size::SMALL))
                .color(palette.text_muted),
            );

            ui.add_space(theme.metrics.gap_md);
            ui.horizontal(|ui| {
                if widgets::Button::new("Open Roblox in Browser")
                    .icon(Icon::External)
                    .tone(widgets::Tone::Neutral)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    state.open_url("https://www.roblox.com/login");
                }

                if widgets::Button::new("Paste from Clipboard")
                    .icon(Icon::Check)
                    .tone(widgets::Tone::Neutral)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    if let Some(clip) = crate::platform::get_clipboard_text() {
                        ui_state.account_cookie_input = clip;
                        state.toasts.success("Pasted from clipboard");
                    }
                }
            });

            ui.add_space(theme.metrics.gap_md);
            ui.label(
                RichText::new("Cookie (.ROBLOSECURITY):")
                    .font(theme::strong(theme::size::BODY))
                    .color(palette.text),
            );
            ui.add_space(theme.metrics.gap_xs);
            widgets::text_field(
                ui,
                &mut ui_state.account_cookie_input,
                "_|WARNING:-DO-NOT-SHARE-THIS.--...",
                420.0,
            );

            if let Some(err) = &ui_state.account_error {
                ui.add_space(theme.metrics.gap_xs);
                ui.label(
                    RichText::new(err)
                        .font(theme::text_style(theme::size::SMALL))
                        .color(palette.danger),
                );
            }

            ui.add_space(theme.metrics.gap_lg);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.metrics.gap_sm;
                if widgets::Button::primary("Save Account")
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    save_cookie = Some(ui_state.account_cookie_input.clone());
                }
                if widgets::Button::new("Cancel")
                    .tone(widgets::Tone::Neutral)
                    .size(widgets::Size::Small)
                    .show(ui)
                    .clicked()
                {
                    close = true;
                }
            });
        });

    if let Some(cookie) = save_cookie {
        if cookie.trim().is_empty() {
            ui_state.account_error = Some("Cookie cannot be empty".into());
        } else {
            state.add_account(&cookie);
            let sanitized = account::sanitize_cookie(&cookie);
            if state.accounts.iter().any(|acc| acc.cookie == sanitized) {
                ui_state.show_manual_account_dialog = false;
                ui_state.account_cookie_input.clear();
                ui_state.account_error = None;
            } else {
                ui_state.account_error = Some("Could not authenticate with this cookie".into());
            }
        }
    }

    if response.should_close() || close {
        ui_state.show_manual_account_dialog = false;
        ui_state.account_error = None;
    }
}
