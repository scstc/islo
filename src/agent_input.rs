//! Shared interaction boundary for every Agent/CLI configuration operation.
//! Ordinary fields belong in commands; secret values stay in this local prompt.
use crate::{clipboard::Shared, i18n::Locale, theme};
use eframe::egui::{self, RichText};
use std::sync::atomic::Ordering;

pub(crate) fn begin(clipboard: &Shared) -> Result<(), String> {
    let _capture = clipboard.lock();
    clipboard
        .private_input
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .map(|_| ())
        .map_err(|_| "Finish or cancel the open secret input first".into())
}

pub(crate) fn finish(clipboard: &Shared, value: &str) {
    clipboard.forget_sensitive(value);
    clipboard.set_private_input(false);
}

// Intentionally neither Debug nor Serialize: the value must never reach IPC or logs.
pub(crate) struct SecretPrompt<'a> {
    pub label: &'a str,
    pub subject: &'a str,
    pub destination: &'a str,
    pub value: &'a mut String,
    pub error: &'a str,
    pub first: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Action {
    Confirm,
    Cancel,
}

pub(crate) fn show(
    ctx: &egui::Context,
    locale: Locale,
    dark: bool,
    prompt: SecretPrompt<'_>,
) -> Option<Action> {
    let mut action = None;
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("islo-local-input"),
        egui::ViewportBuilder::default()
            .with_title(locale.text("islo安全输入"))
            .with_icon(theme::app_icon())
            .with_inner_size([420.0, 260.0])
            .with_resizable(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_active(true),
        |ctx, _| {
            if prompt.first {
                ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
            }
            let cancelled =
                ctx.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape));
            egui::CentralPanel::default()
                .frame(
                    egui::Frame::central_panel(&ctx.style())
                        .inner_margin(20.0)
                        .fill(if dark {
                            egui::Color32::from_rgb(25, 25, 28)
                        } else {
                            egui::Color32::from_rgb(248, 249, 251)
                        }),
                )
                .show(ctx, |ui| {
                    ui.style_mut().visuals = if dark {
                        egui::Visuals::dark()
                    } else {
                        egui::Visuals::light()
                    };
                    ui.spacing_mut().item_spacing = egui::vec2(10.0, 8.0);
                    ui.label(RichText::new(prompt.label).size(20.0).strong());
                    ui.add(egui::Label::new(RichText::new(prompt.subject).strong()).truncate())
                        .on_hover_text(prompt.subject);
                    ui.add(
                        egui::Label::new(RichText::new(prompt.destination).size(11.0)).truncate(),
                    )
                    .on_hover_text(prompt.destination);
                    ui.add_space(4.0);
                    let response = ui.add_sized(
                        [ui.available_width(), 32.0],
                        egui::TextEdit::singleline(prompt.value)
                            .password(true)
                            .char_limit(4096)
                            .desired_width(f32::INFINITY)
                            .hint_text(prompt.label),
                    );
                    if prompt.first {
                        response.request_focus();
                    }
                    if response.lost_focus()
                        && ctx.input(|i| i.key_pressed(egui::Key::Enter))
                        && !prompt.value.is_empty()
                    {
                        action = Some(Action::Confirm);
                    }
                    if !prompt.error.is_empty() {
                        egui::ScrollArea::vertical()
                            .max_height(28.0)
                            .show(ui, |ui| {
                                ui.label(
                                    RichText::new(locale.status(prompt.error))
                                        .small()
                                        .color(egui::Color32::from_rgb(220, 86, 86)),
                                );
                            });
                    }
                    ui.add_space(4.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 32.0),
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui
                                .add_enabled(
                                    !prompt.value.is_empty(),
                                    egui::Button::new(locale.text("确认"))
                                        .min_size(egui::vec2(100.0, 32.0)),
                                )
                                .clicked()
                            {
                                action = Some(Action::Confirm);
                            }
                            if ui
                                .add_sized([90.0, 32.0], egui::Button::new(locale.text("取消")))
                                .clicked()
                            {
                                action = Some(Action::Cancel);
                            }
                        },
                    );
                    ui.label(
                        RichText::new(locale.text("仅在本机保存，不会返回给 Agent。")).size(10.0),
                    );
                });
            if cancelled {
                action = Some(Action::Cancel);
            }
        },
    );
    action
}
