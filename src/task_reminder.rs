//! A separate, persistent fullscreen task prompt; island visibility never gates it.
use crate::{app::App, model, theme};
use chrono::{Duration, Local};
use eframe::egui::{self, Align, Color32, FontId, Layout, RichText, Vec2};
use std::path::Path;

#[derive(Default)]
pub(crate) struct State {
    visible: bool,
    error: String,
    suppressed_snapshot: Option<String>,
    art: Option<Art>,
}

struct Art {
    texture: egui::TextureHandle,
    variant: usize,
    started: f64,
}
const ART_BYTES: [&[u8]; 3] = [
    include_bytes!("../assets/meteocons/clear-day-sheet.png"),
    include_bytes!("../assets/meteocons/partly-cloudy-day-sheet.png"),
    include_bytes!("../assets/meteocons/starry-night-sheet.png"),
];
impl Art {
    fn new(ctx: &egui::Context) -> Self {
        let variant = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|t| t.as_nanos() as usize % 3)
            .unwrap_or(0);
        let image = image::load_from_memory(ART_BYTES[variant])
            .expect("bundled Meteocons atlas")
            .into_rgba8();
        let image = egui::ColorImage::from_rgba_unmultiplied(
            [image.width() as usize, image.height() as usize],
            image.as_raw(),
        );
        Self {
            texture: ctx.load_texture(
                format!("meteocons-{variant}"),
                image,
                egui::TextureOptions::LINEAR,
            ),
            variant,
            started: ctx.input(|i| i.time),
        }
    }
    fn uv(time: f64) -> egui::Rect {
        let frame = ((time.max(0.0) * 12.0) as usize) % 72;
        let x = (frame % 8) as f32 / 8.0;
        let y = (frame / 8) as f32 / 9.0;
        egui::Rect::from_min_max(egui::pos2(x, y), egui::pos2(x + 1.0 / 8.0, y + 1.0 / 9.0))
    }
    fn background(&self, ui: &egui::Ui) {
        let rect = ui.max_rect();
        let colors = match self.variant {
            0 => [
                Color32::from_rgb(20, 43, 62),
                Color32::from_rgb(38, 42, 66),
                Color32::from_rgb(25, 64, 72),
                Color32::from_rgb(13, 33, 49),
            ],
            1 => [
                Color32::from_rgb(27, 35, 76),
                Color32::from_rgb(24, 62, 87),
                Color32::from_rgb(39, 56, 94),
                Color32::from_rgb(27, 34, 64),
            ],
            _ => [
                Color32::from_rgb(27, 24, 63),
                Color32::from_rgb(49, 31, 75),
                Color32::from_rgb(27, 47, 77),
                Color32::from_rgb(15, 24, 48),
            ],
        };
        let mut mesh = egui::Mesh::default();
        for (position, color) in [
            rect.left_top(),
            rect.right_top(),
            rect.right_bottom(),
            rect.left_bottom(),
        ]
        .into_iter()
        .zip(colors)
        {
            mesh.colored_vertex(position, color);
        }
        mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
        ui.painter().add(egui::Shape::mesh(mesh));
        for (center, color) in [
            (
                egui::pos2(
                    rect.left() + rect.width() * 0.2,
                    rect.top() + rect.height() * 0.25,
                ),
                Color32::from_rgb(242, 178, 93),
            ),
            (
                egui::pos2(
                    rect.right() - rect.width() * 0.15,
                    rect.bottom() - rect.height() * 0.15,
                ),
                Color32::from_rgb(88, 155, 226),
            ),
        ] {
            for layer in (1..=12).rev() {
                ui.painter().circle_filled(
                    center,
                    rect.width().min(rect.height()) * 0.025 * layer as f32,
                    color.gamma_multiply(0.008),
                );
            }
        }
        let card = egui::Rect::from_center_size(
            rect.center(),
            egui::vec2(rect.width().min(960.0), (rect.height() - 24.0).min(620.0)),
        );
        ui.painter()
            .rect_filled(card, 28.0, Color32::from_rgba_unmultiplied(16, 25, 43, 145));
        ui.painter().rect_stroke(
            card,
            28.0,
            egui::Stroke::new(1.0_f32, Color32::from_white_alpha(22)),
            egui::StrokeKind::Inside,
        );
    }
    fn draw(&self, ui: &mut egui::Ui) {
        let side = (ui.available_height() * 0.23).clamp(96.0, 200.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::hover());
        let margin = if self.variant == 2 { 56.0 } else { 18.0 };
        let uv = Self::uv(ui.input(|i| i.time) - self.started)
            .shrink2(egui::vec2(margin / 1792.0, margin / 2016.0));
        ui.painter()
            .image(self.texture.id(), rect, uv, Color32::WHITE);
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(83));
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Complete,
    Snooze,
    Dismiss,
}

pub(crate) fn respond(
    todos: &mut model::TodoList,
    index: usize,
    snapshot: &str,
    action: Action,
    now: chrono::DateTime<Local>,
    path: &Path,
) -> Result<(), String> {
    if todos.serialize() != snapshot {
        return Err("待办已变化，请重新操作".into());
    }
    let mut next = todos.clone();
    let task = next
        .items
        .get_mut(index)
        .filter(|t| t.reminder_pending)
        .ok_or("待办已变化，请重新操作")?;
    task.reminder_pending = false;
    task.snoozed_until = None;
    match action {
        Action::Complete => task.done_on = Some(now.format("%Y-%m-%d").to_string()),
        Action::Snooze => {
            task.snoozed_until = Some(
                (now + Duration::minutes(10))
                    .format("%Y-%m-%d %H:%M")
                    .to_string(),
            )
        }
        Action::Dismiss => {}
    }
    next.save_to(path)
        .map_err(|e| format!("提醒处理未保存：{e}"))?;
    *todos = next;
    Ok(())
}

impl App {
    pub fn task_reminder_ui(&mut self, ctx: &egui::Context) {
        if self.clipboard.private_input_active() {
            self.task_reminder.visible = false;
            return;
        }
        let today = Local::now().format("%Y-%m-%d").to_string();
        let pending: Vec<_> = self
            .todos
            .items
            .iter()
            .enumerate()
            .filter(|(_, t)| t.reminder_pending && !t.done_for(&today))
            .map(|(i, _)| i)
            .collect();
        if !self.settings.fullscreen_task_reminders
            || !self.settings.notifications
            || pending.is_empty()
        {
            self.task_reminder.visible = false;
            self.task_reminder.error.clear();
            self.task_reminder.suppressed_snapshot = None;
            self.task_reminder.art = None;
            return;
        }
        let snapshot = self.todos.serialize();
        if self.task_reminder.suppressed_snapshot.as_deref() == Some(snapshot.as_str()) {
            self.task_reminder.visible = false;
            return;
        }
        let first = !self.task_reminder.visible;
        self.task_reminder.visible = true;
        if first || self.task_reminder.art.is_none() {
            self.task_reminder.art = Some(Art::new(ctx));
        }
        let index = pending[0];
        let title = self.todos.items[index].text.clone();
        let due = self.todos.items[index].due.clone().unwrap_or_default();
        let locale = self.settings.locale();
        let busy = self.adding || self.editing.is_some() || self.task_dialog.is_some();
        let mut action = None;
        let mut builder = egui::ViewportBuilder::default()
            .with_title(locale.text("islo待办提醒"))
            .with_icon(theme::app_icon())
            .with_decorations(false)
            .with_resizable(false)
            .with_fullscreen(true)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            .with_active(true);
        if let Some(display) =
            crate::display_layout::selected(&self.displays, &self.settings.display_id)
        {
            // Window creation chooses the monitor from its initial position, before fullscreen.
            let scale = ctx.pixels_per_point();
            builder = builder.with_position(egui::pos2(
                display.bounds.left as f32 / scale,
                display.bounds.top as f32 / scale,
            ));
        }
        ctx.show_viewport_immediate(
            egui::ViewportId::from_hash_of("islo-task-reminder"),
            builder,
            |ctx, _| {
                crate::i18n::set_locale(ctx, locale);
                if first {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                    ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
                }
                if ctx.input(|i| i.viewport().close_requested() || i.key_pressed(egui::Key::Escape))
                {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    action = Some(Action::Dismiss);
                }
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::NONE
                            .fill(Color32::from_rgb(16, 20, 25))
                            .inner_margin(32.0),
                    )
                    .show(ctx, |ui| {
                        let art = self.task_reminder.art.as_ref().unwrap();
                        art.background(ui);
                        let accent = Color32::from_rgb(104, 224, 199);
                        ui.visuals_mut().override_text_color =
                            Some(Color32::from_rgb(243, 246, 248));
                        ui.spacing_mut().item_spacing = Vec2::new(12.0, 16.0);
                        ui.with_layout(Layout::top_down(Align::Center), |ui| {
                            ui.add_space(((ui.available_height() - 600.0) * 0.5).max(0.0));
                            ui.label(
                                RichText::new("islo")
                                    .font(FontId::proportional(18.0))
                                    .color(accent),
                            );
                            ui.add_space(8.0);
                            art.draw(ui);
                            ui.label(
                                RichText::new(locale.text("待办到时间了"))
                                    .font(FontId::proportional(28.0))
                                    .color(accent),
                            );
                            let width = ui.available_width().min(880.0);
                            egui::ScrollArea::vertical()
                                .id_salt("fullscreen-task-title")
                                .max_height((ui.available_height() * 0.30).max(70.0))
                                .show(ui, |ui| {
                                    ui.set_max_width(width);
                                    ui.add(
                                        egui::Label::new(RichText::new(&title).font(theme::font(
                                            if title.chars().count() > 80 {
                                                32.0
                                            } else {
                                                44.0
                                            },
                                            theme::W::Semibold,
                                        )))
                                        .wrap(),
                                    );
                                });
                            ui.label(
                                RichText::new(&due)
                                    .size(16.0)
                                    .color(Color32::from_gray(175)),
                            );
                            ui.add_space(12.0);
                            // The snapshot is also the widget identity: a concurrent CLI edit cannot
                            // turn an old pointer-down into a completion of a different task.
                            ui.push_id(&snapshot, |ui| {
                                ui.horizontal_wrapped(|ui| {
                                    let button_width =
                                        ((ui.available_width() - 24.0) / 3.0).clamp(100.0, 190.0);
                                    let group_width = button_width * 3.0 + 24.0;
                                    ui.add_space(
                                        ((ui.available_width() - group_width) * 0.5).max(0.0),
                                    );
                                    let complete = egui::Button::new(
                                        RichText::new(locale.text("完成待办"))
                                            .size(17.0)
                                            .color(Color32::from_rgb(13, 35, 31)),
                                    )
                                    .fill(accent)
                                    .min_size(Vec2::new(button_width, 54.0));
                                    if ui.add_enabled(!busy, complete).clicked() {
                                        action = Some(Action::Complete);
                                    }
                                    let snooze = egui::Button::new(
                                        RichText::new(locale.text("10 分钟后提醒")).size(16.0),
                                    )
                                    .fill(Color32::from_rgb(44, 52, 61))
                                    .min_size(Vec2::new(button_width, 54.0));
                                    if ui.add_enabled(!busy, snooze).clicked() {
                                        action = Some(Action::Snooze);
                                    }
                                    if ui
                                        .add(
                                            egui::Button::new(
                                                RichText::new(locale.text("暂不处理")).size(16.0),
                                            )
                                            .fill(Color32::from_rgb(30, 36, 44))
                                            .min_size(Vec2::new(button_width, 54.0)),
                                        )
                                        .clicked()
                                    {
                                        action = Some(Action::Dismiss);
                                    }
                                });
                            });
                            ui.add_space(8.0);
                            ui.label(
                                RichText::new(locale.text("提醒会保留，直到你选择处理方式。"))
                                    .size(14.0)
                                    .color(Color32::from_gray(170)),
                            );
                            if pending.len() > 1 {
                                ui.label(locale.format(
                                    "还有 {0} 项待办待处理",
                                    &[("0", (pending.len() - 1).to_string())],
                                ));
                            }
                            if busy {
                                ui.label(locale.text("请先完成或取消待办编辑，再完成或延后提醒。"));
                            }
                            if !self.task_reminder.error.is_empty() {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(locale.status(&self.task_reminder.error))
                                            .color(Color32::from_rgb(255, 154, 154)),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                    });
            },
        );
        if let Some(action) = action {
            match respond(
                &mut self.todos,
                index,
                &snapshot,
                action,
                Local::now(),
                &model::config_dir().join("todo.txt"),
            ) {
                Ok(()) => {
                    self.task_reminder.error.clear();
                    self.task_reminder.visible = false;
                    self.alerts
                        .retain(|a| !(a.page == 4 && a.title == "待办到时间了" && a.body == title));
                    if self.notice.as_ref().is_some_and(|a| {
                        a.page == 4 && a.title == "待办到时间了" && a.body == title
                    }) {
                        self.notice = None;
                    }
                    ctx.request_repaint();
                }
                Err(error) => {
                    if matches!(action, Action::Dismiss) {
                        // A failed disk must never trap the user in a fullscreen window.
                        // Keep the pending state on disk so a restart can recover it.
                        self.task_reminder.suppressed_snapshot = Some(snapshot);
                        self.task_reminder.visible = false;
                        self.say(self.settings.locale().status(&error));
                        ctx.request_repaint();
                    } else {
                        self.task_reminder.error = error;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn meteocons_frames_animate_and_loop_without_reuploading_textures() {
        assert_ne!(Art::uv(0.0), Art::uv(0.2));
        assert_eq!(Art::uv(0.0), Art::uv(6.0));
        for bytes in ART_BYTES {
            let atlas = image::load_from_memory(bytes).unwrap().into_rgba8();
            assert_eq!(atlas.dimensions(), (1792, 2016));
            let first = image::imageops::crop_imm(&atlas, 0, 0, 224, 224).to_image();
            let later = image::imageops::crop_imm(&atlas, 224 * 3, 224, 224, 224).to_image();
            assert_ne!(
                first.as_raw(),
                later.as_raw(),
                "Bundled animated resources must contain actual changing frames"
            );
        }
    }

    fn pending_task() -> model::TodoItem {
        model::TodoItem {
            due: Some("2026-11-06 23:50".into()),
            repeat: model::REPEAT_WORKDAYS,
            reminded_on: Some("2026-11-06".into()),
            reminder_pending: true,
            ..model::TodoItem::new("Report | details")
        }
    }

    #[test]
    fn fullscreen_pending_and_snooze_survive_restart_without_changing_recurrence() {
        let original = model::TodoList {
            items: vec![pending_task()],
        };
        let loaded = model::TodoList::parse(&original.serialize());
        assert!(loaded.items[0].reminder_pending);
        assert!(
            !loaded.items[0].reminder_due("2026-11-06 23:59"),
            "An unacknowledged prompt must not retrigger within the same occurrence"
        );
        assert!(
            loaded.items[0].reminder_due("2026-11-09 23:59"),
            "Pending fullscreen state must not disable the task's future weekly schedule"
        );
        let path = std::env::temp_dir().join(format!(
            "islo-fullscreen-{}-{}.txt",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        let mut todos = loaded;
        let snapshot = todos.serialize();
        // Friday -> Saturday: snooze must work even if Saturday is outside the weekly mask.
        let now = Local
            .with_ymd_and_hms(2026, 11, 6, 23, 55, 0)
            .single()
            .unwrap();
        respond(&mut todos, 0, &snapshot, Action::Snooze, now, &path).unwrap();
        let loaded = model::TodoList::load_from(&path).unwrap();
        assert_eq!(loaded.items[0].repeat, model::REPEAT_WORKDAYS);
        assert_eq!(loaded.items[0].due.as_deref(), Some("2026-11-06 23:50"));
        assert!(!loaded.items[0].reminder_pending);
        assert_eq!(
            loaded.items[0].snoozed_until.as_deref(),
            Some("2026-11-07 00:05")
        );
        assert!(!loaded.items[0].reminder_due("2026-11-07 00:04"));
        assert!(loaded.items[0].reminder_due("2026-11-07 00:05"));
        let mut daily = loaded.items[0].clone();
        daily.repeat = model::REPEAT_ALL;
        daily.mark_reminded("2026-11-07 00:05");
        assert!(
            daily.reminder_due("2026-11-07 23:50"),
            "A snoozed previous-day occurrence must not consume today's regular reminder"
        );
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn fullscreen_actions_do_not_complete_the_wrong_task_or_lose_failed_writes() {
        let mut todos = model::TodoList {
            items: vec![pending_task()],
        };
        let snapshot = todos.serialize();
        let root = std::env::temp_dir().join(format!(
            "islo-fullscreen-fail-{}-{}",
            std::process::id(),
            Local::now().timestamp_nanos_opt().unwrap()
        ));
        std::fs::create_dir(&root).unwrap();
        let now = Local::now();
        assert!(respond(&mut todos, 0, &snapshot, Action::Complete, now, &root).is_err());
        assert!(todos.items[0].reminder_pending);
        assert!(todos.items[0].done_on.is_none());
        todos
            .items
            .insert(0, model::TodoItem::new("A different task"));
        let path = root.join("tasks.txt");
        assert!(respond(&mut todos, 0, &snapshot, Action::Complete, now, &path).is_err());
        assert!(!path.exists());
        assert!(todos.items[0].done_on.is_none());
        let snapshot = todos.serialize();
        respond(&mut todos, 1, &snapshot, Action::Complete, now, &path).unwrap();
        assert!(todos.items[1].done_for(&now.format("%Y-%m-%d").to_string()));
        assert!(!todos.items[1].reminder_pending);
        std::fs::remove_file(path).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
