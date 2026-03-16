use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use eframe::egui;
use time::OffsetDateTime;

use crate::{
    config::{AppConfig, ThemePreset},
    status::{PromotionState, active_window_ends_at, next_two_x_starts_at, status_at},
};

// ── Theme ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Theme {
    bg: egui::Color32,
    surface: egui::Color32,
    accent: egui::Color32,
    accent_dim: egui::Color32,
    inactive: egui::Color32,
    text: egui::Color32,
    secondary: egui::Color32,
    dim: egui::Color32,
    separator: egui::Color32,
    hover: egui::Color32,
}

impl Theme {
    fn from_config(cfg: &crate::config::ThemeConfig) -> Self {
        let mut theme = match cfg.preset {
            ThemePreset::ClaudeDark => Self {
                bg: c(0x0f, 0x10, 0x19),
                surface: c(0x16, 0x18, 0x24),
                accent: c(0xD2, 0x95, 0x6E),
                accent_dim: c(0x9A, 0x6E, 0x4E),
                inactive: c(0x55, 0x58, 0x68),
                text: c(0xf0, 0xf0, 0xf0),
                secondary: c(0x77, 0x7a, 0x8a),
                dim: c(0x44, 0x46, 0x54),
                separator: c(0x1e, 0x20, 0x2e),
                hover: c(0x1c, 0x1e, 0x2c),
            },
            ThemePreset::ClaudeLight => Self {
                bg: c(0xf2, 0xf0, 0xee),
                surface: c(0xff, 0xff, 0xff),
                accent: c(0xC4, 0x7D, 0x56),
                accent_dim: c(0xA0, 0x66, 0x44),
                inactive: c(0xaa, 0xaa, 0xb0),
                text: c(0x1a, 0x1a, 0x24),
                secondary: c(0x6c, 0x70, 0x80),
                dim: c(0xbb, 0xbb, 0xc4),
                separator: c(0xd8, 0xd6, 0xd4),
                hover: c(0xe8, 0xe6, 0xe4),
            },
            ThemePreset::Midnight => Self {
                bg: c(0x0a, 0x0a, 0x14),
                surface: c(0x12, 0x12, 0x20),
                accent: c(0x7C, 0x6A, 0xE8),
                accent_dim: c(0x5A, 0x4C, 0xB0),
                inactive: c(0x48, 0x48, 0x60),
                text: c(0xe0, 0xe0, 0xee),
                secondary: c(0x68, 0x68, 0x88),
                dim: c(0x38, 0x38, 0x50),
                separator: c(0x1a, 0x1a, 0x2c),
                hover: c(0x18, 0x18, 0x28),
            },
            ThemePreset::Sunset => Self {
                bg: c(0x14, 0x0c, 0x12),
                surface: c(0x1e, 0x14, 0x1a),
                accent: c(0xE8, 0x6A, 0x7C),
                accent_dim: c(0xB0, 0x4E, 0x5C),
                inactive: c(0x60, 0x48, 0x54),
                text: c(0xf0, 0xe4, 0xe8),
                secondary: c(0x90, 0x78, 0x80),
                dim: c(0x44, 0x34, 0x3c),
                separator: c(0x2a, 0x1c, 0x24),
                hover: c(0x28, 0x1a, 0x22),
            },
        };

        if let Some(ref hex) = cfg.accent_hex
            && let Some(parsed) = parse_hex_color(hex)
        {
            theme.accent = parsed;
            theme.accent_dim = dim_color(parsed);
        }

        theme
    }

    fn apply_to_egui(&self, ctx: &egui::Context) {
        let mut v = egui::Visuals::dark();
        v.panel_fill = self.bg;
        v.window_fill = self.bg;
        v.faint_bg_color = self.surface;
        v.widgets.noninteractive.bg_fill = self.surface;
        v.widgets.inactive.bg_fill = self.surface;
        v.widgets.hovered.bg_fill = self.hover;
        v.widgets.active.bg_fill = self.accent_dim;
        v.override_text_color = Some(self.text);
        v.window_corner_radius = egui::CornerRadius::same(16);
        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.5, self.secondary);
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, self.accent);
        v.widgets.active.fg_stroke = egui::Stroke::new(2.0, self.accent);
        ctx.set_visuals(v);
    }
}

const fn c(r: u8, g: u8, b: u8) -> egui::Color32 {
    egui::Color32::from_rgb(r, g, b)
}

fn dim_color(col: egui::Color32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (col.r() as u16 * 3 / 4) as u8,
        (col.g() as u16 * 3 / 4) as u8,
        (col.b() as u16 * 3 / 4) as u8,
    )
}

fn parse_hex_color(hex: &str) -> Option<egui::Color32> {
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some(egui::Color32::from_rgb(r, g, b))
}

// ── Public entry (tray process) ──────────────────────────────────────────────

pub fn show_popup(popup_open: Arc<AtomicBool>) {
    if popup_open.load(Ordering::SeqCst) {
        return;
    }
    popup_open.store(true, Ordering::SeqCst);
    let popup_open_clone = Arc::clone(&popup_open);

    std::thread::spawn(move || {
        let exe = std::env::current_exe().expect("cannot resolve own exe path");
        let status = std::process::Command::new(exe).arg("--popup").status();
        if let Err(e) = status {
            eprintln!("failed to spawn popup process: {e}");
        }
        popup_open_clone.store(false, Ordering::SeqCst);
    });
}

// ── Subprocess entry ─────────────────────────────────────────────────────────

pub fn run_popup() -> Result<()> {
    let config = AppConfig::load().unwrap_or_default();
    let config = Arc::new(Mutex::new(config));

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([280.0, 520.0])
            .with_decorations(false)
            .with_always_on_top()
            .with_drag_and_drop(true),
        ..Default::default()
    };

    let config_clone = Arc::clone(&config);
    eframe::run_native(
        "Claude 2x",
        options,
        Box::new(move |cc| {
            let app = PopupApp::new(config_clone);
            app.theme.apply_to_egui(&cc.egui_ctx);
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("{e}"))
}

// ── App state ────────────────────────────────────────────────────────────────

struct PopupApp {
    config: Arc<Mutex<AppConfig>>,
    local_config: AppConfig,
    theme: Theme,
    dirty: bool,
}

impl PopupApp {
    fn new(config: Arc<Mutex<AppConfig>>) -> Self {
        let local_config = config.lock().unwrap().clone();
        let theme = Theme::from_config(&local_config.theme);
        Self {
            config,
            local_config,
            theme,
            dirty: false,
        }
    }

    fn rebuild_theme(&mut self, ctx: &egui::Context) {
        self.theme = Theme::from_config(&self.local_config.theme);
        self.theme.apply_to_egui(ctx);
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

impl eframe::App for PopupApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(std::time::Duration::from_millis(200));

        let now_utc = OffsetDateTime::now_utc();
        let snapshot = status_at(now_utc);
        let t = self.theme.clone();

        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(t.bg)
                    .corner_radius(egui::CornerRadius::same(16))
                    .inner_margin(egui::Margin::same(0))
                    .stroke(egui::Stroke::new(1.0, t.separator)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    // ── Top area with subtle gradient feel ──
                    egui::Frame::new()
                        .fill(t.bg)
                        .inner_margin(egui::Margin::symmetric(18, 20))
                        .show(ui, |ui| {
                            // Header: logo + name + close.
                            let title_resp = ui.horizontal(|ui| {
                                // Logo square.
                                let (logo_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(22.0, 22.0),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    logo_rect,
                                    egui::CornerRadius::same(6),
                                    t.accent,
                                );
                                ui.painter().text(
                                    logo_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "C",
                                    egui::FontId::proportional(11.0),
                                    c(0x0e, 0x0e, 0x0e),
                                );

                                ui.add_space(8.0);
                                ui.label(
                                    egui::RichText::new("Claude 2x")
                                        .size(13.0)
                                        .strong()
                                        .color(t.text.gamma_multiply(0.7)),
                                );

                                // Close button.
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let btn = ui.add_sized(
                                            [28.0, 28.0],
                                            egui::Button::new(
                                                egui::RichText::new("\u{2715}")
                                                    .size(13.0)
                                                    .color(t.secondary),
                                            )
                                            .corner_radius(egui::CornerRadius::same(6))
                                            .fill(egui::Color32::TRANSPARENT)
                                            .stroke(egui::Stroke::NONE),
                                        );
                                        if btn.clicked() {
                                            ctx.send_viewport_cmd(
                                                egui::ViewportCommand::Close,
                                            );
                                        }
                                    },
                                );
                            });
                            if title_resp
                                .response
                                .interact(egui::Sense::drag())
                                .dragged()
                            {
                                ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                            }

                            ui.add_space(16.0);

                            // Status row: chip + "2x ends in".
                            ui.horizontal(|ui| {
                                // Status chip.
                                let chip_text = if snapshot.is_active {
                                    "Active"
                                } else {
                                    match snapshot.state {
                                        PromotionState::BeforeStart => "Soon",
                                        PromotionState::Ended => "Ended",
                                        _ => "Standard",
                                    }
                                };
                                let chip_color = if snapshot.is_active {
                                    t.accent
                                } else {
                                    t.inactive
                                };

                                let chip_w = 70.0;
                                let chip_h = 22.0;
                                let (chip_rect, _) = ui.allocate_exact_size(
                                    egui::vec2(chip_w, chip_h),
                                    egui::Sense::hover(),
                                );
                                ui.painter().rect_filled(
                                    chip_rect,
                                    egui::CornerRadius::same(6),
                                    chip_color.gamma_multiply(0.12),
                                );
                                // Dot.
                                let dot_center = egui::pos2(
                                    chip_rect.left() + 12.0,
                                    chip_rect.center().y,
                                );
                                ui.painter()
                                    .circle_filled(dot_center, 3.0, chip_color);
                                // Label.
                                ui.painter().text(
                                    egui::pos2(
                                        chip_rect.left() + 22.0,
                                        chip_rect.center().y,
                                    ),
                                    egui::Align2::LEFT_CENTER,
                                    chip_text,
                                    egui::FontId::proportional(11.0),
                                    chip_color,
                                );

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let cd = countdown_text(now_utc, snapshot.state);
                                        ui.label(
                                            egui::RichText::new(&cd.label)
                                                .size(11.0)
                                                .color(t.dim),
                                        );
                                    },
                                );
                            });

                            ui.add_space(12.0);

                            // ── Big countdown digits ──
                            let cd = countdown_text(now_utc, snapshot.state);
                            draw_countdown(ui, &cd.value, &t);

                            ui.add_space(6.0);

                            // ── Progress bar ──
                            let bar_h = 3.0;
                            let (bar_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), bar_h),
                                egui::Sense::hover(),
                            );
                            ui.painter().rect_filled(
                                bar_rect,
                                egui::CornerRadius::same(2),
                                t.text.gamma_multiply(0.04),
                            );
                            let progress = compute_progress(now_utc, snapshot.state);
                            if progress > 0.001 {
                                let fill_rect = egui::Rect::from_min_size(
                                    bar_rect.min,
                                    egui::vec2(bar_rect.width() * progress, bar_h),
                                );
                                ui.painter().rect_filled(
                                    fill_rect,
                                    egui::CornerRadius::same(2),
                                    t.accent,
                                );
                            }

                            ui.add_space(10.0);

                            // ── Next 2x row ──
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Next 2x")
                                        .size(11.0)
                                        .color(t.dim),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let next = next_two_x_value(now_utc, snapshot.state);
                                        ui.label(
                                            egui::RichText::new(&next)
                                                .size(11.0)
                                                .color(t.secondary),
                                        );
                                    },
                                );
                            });
                        });

                    // ── Separator ──
                    let sep_rect = egui::Rect::from_min_size(
                        ui.cursor().min + egui::vec2(18.0, 0.0),
                        egui::vec2(ui.available_width() - 36.0, 1.0),
                    );
                    ui.painter().rect_filled(
                        sep_rect,
                        egui::CornerRadius::ZERO,
                        t.text.gamma_multiply(0.04),
                    );
                    ui.add_space(1.0);

                    // ── Alerts section ──
                    egui::Frame::new()
                        .fill(t.bg)
                        .inner_margin(egui::Margin::symmetric(18, 12))
                        .show(ui, |ui| {
                            section_title(ui, "ALERTS", &t);
                            ui.add_space(8.0);

                            pill_label(ui, "Before 2x ends", &t);
                            ui.add_space(4.0);
                            if pill_row(
                                ui,
                                &mut self.local_config.notifications.before_end_minutes,
                                &t,
                            ) {
                                self.dirty = true;
                            }

                            ui.add_space(8.0);

                            pill_label(ui, "Before 2x starts", &t);
                            ui.add_space(4.0);
                            if pill_row(
                                ui,
                                &mut self.local_config.notifications.before_start_minutes,
                                &t,
                            ) {
                                self.dirty = true;
                            }

                            ui.add_space(10.0);

                            // Sound toggle row.
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Sound")
                                        .size(12.0)
                                        .color(t.secondary),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if toggle_switch(
                                            ui,
                                            &mut self.local_config.notifications.sound,
                                            &t,
                                        ) {
                                            self.dirty = true;
                                        }
                                    },
                                );
                            });
                        });

                    // ── Separator ──
                    let sep_rect2 = egui::Rect::from_min_size(
                        ui.cursor().min + egui::vec2(18.0, 0.0),
                        egui::vec2(ui.available_width() - 36.0, 1.0),
                    );
                    ui.painter().rect_filled(
                        sep_rect2,
                        egui::CornerRadius::ZERO,
                        t.text.gamma_multiply(0.04),
                    );
                    ui.add_space(1.0);

                    // ── Theme section ──
                    egui::Frame::new()
                        .fill(t.bg)
                        .inner_margin(egui::Margin::symmetric(18, 12))
                        .show(ui, |ui| {
                            section_title(ui, "THEME", &t);
                            ui.add_space(8.0);

                            let mut theme_changed = false;
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;

                                let presets = [
                                    (ThemePreset::ClaudeDark, c(0xD2, 0x95, 0x6E)),
                                    (ThemePreset::ClaudeLight, c(0xc4, 0xc0, 0xb8)),
                                    (ThemePreset::Midnight, c(0x7C, 0x6A, 0xE8)),
                                    (ThemePreset::Sunset, c(0xE8, 0x6A, 0x7C)),
                                ];

                                for (preset, color) in presets {
                                    let is_sel =
                                        self.local_config.theme.preset == preset;
                                    let size = 22.0;
                                    let (rect, resp) = ui.allocate_exact_size(
                                        egui::vec2(size, size),
                                        egui::Sense::click(),
                                    );
                                    let ctr = rect.center();

                                    ui.painter()
                                        .circle_filled(ctr, size / 2.0, color);

                                    if is_sel {
                                        ui.painter().circle_stroke(
                                            ctr,
                                            size / 2.0 + 3.0,
                                            egui::Stroke::new(
                                                2.0,
                                                t.text.gamma_multiply(0.4),
                                            ),
                                        );
                                    }

                                    if resp.hovered() && !is_sel {
                                        ui.painter().circle_stroke(
                                            ctr,
                                            size / 2.0 + 3.0,
                                            egui::Stroke::new(
                                                1.0,
                                                t.text.gamma_multiply(0.15),
                                            ),
                                        );
                                    }

                                    if resp.clicked() && !is_sel {
                                        self.local_config.theme.preset = preset;
                                        self.local_config.theme.accent_hex = None;
                                        theme_changed = true;
                                        self.dirty = true;
                                    }
                                }
                            });

                            if theme_changed {
                                self.rebuild_theme(ctx);
                            }
                        });

                    ui.add_space(8.0);
                });

                // Auto-save.
                if self.dirty {
                    self.dirty = false;
                    *self.config.lock().unwrap() = self.local_config.clone();
                    if let Err(e) = self.local_config.save() {
                        eprintln!("failed to save config: {e:#}");
                    }
                }
            });
    }
}

// ── UI components ────────────────────────────────────────────────────────────

fn section_title(ui: &mut egui::Ui, label: &str, theme: &Theme) {
    ui.label(
        egui::RichText::new(label)
            .size(10.0)
            .color(theme.dim)
            .strong(),
    );
}

fn pill_label(ui: &mut egui::Ui, label: &str, theme: &Theme) {
    ui.label(
        egui::RichText::new(label)
            .size(10.0)
            .color(theme.dim),
    );
}

const THRESHOLDS: &[u32] = &[5, 15, 30, 60];

fn pill_row(ui: &mut egui::Ui, selected: &mut Vec<u32>, theme: &Theme) -> bool {
    let mut changed = false;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 5.0;
        let pill_w = (ui.available_width() - 15.0) / 4.0;

        for &mins in THRESHOLDS {
            let is_on = selected.contains(&mins);
            let label = if mins >= 60 {
                format!("{}h", mins / 60)
            } else {
                format!("{}m", mins)
            };

            let pill_h = 30.0;
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(pill_w, pill_h), egui::Sense::click());
            let rounding = egui::CornerRadius::same(8);
            let hovered = response.hovered();

            let (fill, text_color) = if is_on {
                (
                    if hovered { theme.accent_dim } else { theme.accent },
                    c(0x0e, 0x0e, 0x0e),
                )
            } else {
                (
                    if hovered {
                        theme.text.gamma_multiply(0.06)
                    } else {
                        theme.text.gamma_multiply(0.03)
                    },
                    theme.secondary,
                )
            };

            ui.painter().rect_filled(rect, rounding, fill);

            if !is_on {
                ui.painter().rect_stroke(
                    rect,
                    rounding,
                    egui::Stroke::new(1.0, theme.text.gamma_multiply(0.06)),
                    egui::StrokeKind::Inside,
                );
            }

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(12.0),
                text_color,
            );

            if response.clicked() {
                if is_on {
                    selected.retain(|&m| m != mins);
                } else {
                    selected.push(mins);
                    selected.sort();
                }
                changed = true;
            }
        }
    });

    changed
}

fn toggle_switch(ui: &mut egui::Ui, value: &mut bool, theme: &Theme) -> bool {
    let w = 32.0;
    let h = 18.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());

    let bg = if *value {
        theme.accent
    } else {
        theme.text.gamma_multiply(0.08)
    };
    ui.painter()
        .rect_filled(rect, egui::CornerRadius::same(9), bg);

    let knob_r = 7.0;
    let knob_x = if *value {
        rect.right() - knob_r - 2.0
    } else {
        rect.left() + knob_r + 2.0
    };
    ui.painter().circle_filled(
        egui::pos2(knob_x, rect.center().y),
        knob_r,
        egui::Color32::WHITE,
    );

    let mut changed = false;
    if response.clicked() {
        *value = !*value;
        changed = true;
    }
    changed
}

fn draw_countdown(ui: &mut egui::Ui, value: &str, theme: &Theme) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        let parts: Vec<&str> = value.split_whitespace().collect();
        for part in &parts {
            if let Some(unit_idx) = part.find(|ch: char| ch.is_alphabetic()) {
                let digits = &part[..unit_idx];
                let unit = &part[unit_idx..];

                ui.label(
                    egui::RichText::new(digits)
                        .size(44.0)
                        .strong()
                        .color(theme.text)
                        .family(egui::FontFamily::Monospace),
                );
                ui.label(
                    egui::RichText::new(unit)
                        .size(13.0)
                        .color(theme.dim),
                );
                ui.add_space(6.0);
            } else {
                ui.label(
                    egui::RichText::new(*part)
                        .size(44.0)
                        .strong()
                        .color(theme.text),
                );
            }
        }
    });
}

// ── Data helpers ─────────────────────────────────────────────────────────────

fn compute_progress(now_utc: OffsetDateTime, state: PromotionState) -> f32 {
    match state {
        PromotionState::TwoX => active_window_ends_at(now_utc)
            .map(|end| {
                let remaining = (end - now_utc).whole_seconds().max(0) as f32;
                (remaining / (6.0 * 3600.0)).clamp(0.0, 1.0)
            })
            .unwrap_or(0.0),
        PromotionState::Standard | PromotionState::BeforeStart => {
            next_two_x_starts_at(now_utc)
                .map(|start| {
                    let remaining = (start - now_utc).whole_seconds().max(0) as f32;
                    (remaining / (6.0 * 3600.0)).clamp(0.0, 1.0)
                })
                .unwrap_or(0.0)
        }
        PromotionState::Ended => 0.0,
    }
}

struct CountdownInfo {
    label: String,
    value: String,
}

fn countdown_text(now_utc: OffsetDateTime, state: PromotionState) -> CountdownInfo {
    match state {
        PromotionState::TwoX => CountdownInfo {
            label: "2x ends in".to_string(),
            value: active_window_ends_at(now_utc)
                .map(|end| format_duration(end - now_utc))
                .unwrap_or_else(|| "--".to_string()),
        },
        PromotionState::Standard | PromotionState::BeforeStart => CountdownInfo {
            label: "2x starts in".to_string(),
            value: next_two_x_starts_at(now_utc)
                .map(|start| format_duration(start - now_utc))
                .unwrap_or_else(|| "--".to_string()),
        },
        PromotionState::Ended => CountdownInfo {
            label: "Promotion".to_string(),
            value: "Ended".to_string(),
        },
    }
}

fn next_two_x_value(now_utc: OffsetDateTime, state: PromotionState) -> String {
    match state {
        PromotionState::TwoX => next_two_x_starts_at(now_utc)
            .map(format_local_timestamp)
            .unwrap_or_else(|| "Last window".to_string()),
        PromotionState::BeforeStart | PromotionState::Standard => next_two_x_starts_at(now_utc)
            .map(format_local_timestamp)
            .unwrap_or_else(|| "N/A".to_string()),
        PromotionState::Ended => "Ended".to_string(),
    }
}

fn format_duration(duration: time::Duration) -> String {
    let total_seconds = duration.whole_seconds().max(0);
    let hours = total_seconds / 3_600;
    let minutes = (total_seconds % 3_600) / 60;
    let seconds = total_seconds % 60;

    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else {
        format!("{minutes}m {seconds:02}s")
    }
}

fn format_local_timestamp(timestamp_utc: OffsetDateTime) -> String {
    let local_offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let local = timestamp_utc.to_offset(local_offset);
    let month = match local.month() {
        time::Month::January => "Jan",
        time::Month::February => "Feb",
        time::Month::March => "Mar",
        time::Month::April => "Apr",
        time::Month::May => "May",
        time::Month::June => "Jun",
        time::Month::July => "Jul",
        time::Month::August => "Aug",
        time::Month::September => "Sep",
        time::Month::October => "Oct",
        time::Month::November => "Nov",
        time::Month::December => "Dec",
    };
    let hour_24 = local.hour();
    let (hour_12, meridiem) = match hour_24 {
        0 => (12, "AM"),
        1..=11 => (hour_24, "AM"),
        12 => (12, "PM"),
        _ => (hour_24 - 12, "PM"),
    };
    format!(
        "{month} {} {:02}:{:02} {meridiem}",
        local.day(),
        hour_12,
        local.minute()
    )
}
