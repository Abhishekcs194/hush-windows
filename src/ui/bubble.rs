use egui::{Color32, Context, Rect, RichText, Rounding, Stroke, Vec2, ViewportBuilder, ViewportId};

use crate::ui::{accent, accent_dim};

const BUBBLE_W: f32 = 420.0;
const BUBBLE_H: f32 = 72.0;
const BAR_COUNT: usize = 42;
const BAR_W: f32 = 3.0;
const BAR_GAP: f32 = 2.0;
const BAR_MAX_H: f32 = 36.0;

#[derive(Debug, Clone, PartialEq)]
pub enum BubblePhase {
    Hidden,
    Idle,
    Recording,
    Processing,
    Done { message: String },
}

pub struct BubbleState {
    pub phase: BubblePhase,
    pub rms_levels: Vec<f32>, // ring buffer of recent RMS values for waveform
    pub progress: Option<f32>, // 0.0–1.0 for determinate progress bar
}

impl BubbleState {
    pub fn new() -> Self {
        Self {
            phase: BubblePhase::Hidden,
            rms_levels: vec![0.0; BAR_COUNT],
            progress: None,
        }
    }

    pub fn push_level(&mut self, level: f32) {
        self.rms_levels.rotate_left(1);
        *self.rms_levels.last_mut().unwrap() = level;
    }

    pub fn reset_levels(&mut self) {
        self.rms_levels.iter_mut().for_each(|v| *v = 0.0);
    }
}

impl Default for BubbleState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn show(ctx: &Context, state: &BubbleState, on_cancel: &mut bool, on_finish: &mut bool) {
    if state.phase == BubblePhase::Hidden {
        return;
    }

    // Position at bottom-center of primary monitor
    let screen = ctx.input(|i| i.screen_rect());
    let x = screen.center().x - BUBBLE_W / 2.0;
    let y = screen.max.y - BUBBLE_H - 24.0;

    let passthrough = !matches!(state.phase, BubblePhase::Recording);

    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("hush_bubble"),
        ViewportBuilder::default()
            .with_title("Hush Bubble")
            .with_decorations(false)
            .with_transparent(true)
            .with_always_on_top()
            .with_taskbar(false)
            .with_inner_size([BUBBLE_W, BUBBLE_H])
            .with_position([x, y])
            .with_mouse_passthrough(passthrough),
        |ctx, _| {
            render_bubble(ctx, state, on_cancel, on_finish);
        },
    );
}

fn render_bubble(
    ctx: &Context,
    state: &BubbleState,
    on_cancel: &mut bool,
    on_finish: &mut bool,
) {
    egui::CentralPanel::default()
        .frame(egui::Frame::none())
        .show(ctx, |ui| {
            let rect = ui.available_rect_before_wrap();
            let painter = ui.painter();

            // Background pill
            painter.rect_filled(rect, Rounding::same(rect.height() / 2.0), pill_bg());

            match &state.phase {
                BubblePhase::Idle => {
                    draw_idle_bar(painter, rect);
                }
                BubblePhase::Recording => {
                    draw_waveform(painter, rect, &state.rms_levels);
                    draw_action_buttons(ui, rect, on_cancel, on_finish);
                }
                BubblePhase::Processing => {
                    draw_processing(ui, rect, state.progress);
                }
                BubblePhase::Done { message } => {
                    draw_done(ui, rect, message);
                }
                BubblePhase::Hidden => {}
            }
        });
}

fn pill_bg() -> Color32 {
    Color32::from_rgba_premultiplied(28, 28, 30, 230)
}

fn draw_idle_bar(painter: &egui::Painter, rect: Rect) {
    let w = rect.width() * 0.4;
    let h = 4.0;
    let r = Rect::from_center_size(rect.center(), Vec2::new(w, h));
    painter.rect_filled(r, Rounding::same(2.0), Color32::from_rgb(80, 80, 85));
}

fn draw_waveform(painter: &egui::Painter, rect: Rect, levels: &[f32]) {
    let total_w = BAR_COUNT as f32 * (BAR_W + BAR_GAP) - BAR_GAP;
    let start_x = rect.center().x - total_w / 2.0;
    let cy = rect.center().y;

    for (i, &level) in levels.iter().enumerate() {
        let normalized = (level * 60.0).min(1.0);
        let bar_h = (normalized * BAR_MAX_H).max(3.0);
        let x = start_x + i as f32 * (BAR_W + BAR_GAP);
        let r = Rect::from_center_size(
            egui::pos2(x + BAR_W / 2.0, cy),
            Vec2::new(BAR_W, bar_h),
        );
        let alpha = ((normalized * 255.0) as u8).max(80);
        painter.rect_filled(r, Rounding::same(BAR_W / 2.0), Color32::from_rgba_premultiplied(247, 69, 161, alpha));
    }
}

fn draw_action_buttons(
    ui: &mut egui::Ui,
    rect: Rect,
    on_cancel: &mut bool,
    on_finish: &mut bool,
) {
    let button_y = rect.center().y;
    let cancel_x = rect.min.x + 24.0;
    let finish_x = rect.max.x - 24.0;

    // Cancel (✗)
    let cancel_rect = Rect::from_center_size(egui::pos2(cancel_x, button_y), Vec2::splat(28.0));
    let cancel_resp = ui.allocate_rect(cancel_rect, egui::Sense::click());
    if cancel_resp.hovered() {
        ui.painter().circle_filled(cancel_rect.center(), 14.0, Color32::from_rgb(60, 60, 65));
    }
    ui.painter().text(
        cancel_rect.center(),
        egui::Align2::CENTER_CENTER,
        "✕",
        egui::FontId::proportional(16.0),
        Color32::from_rgb(200, 100, 100),
    );
    if cancel_resp.clicked() {
        *on_cancel = true;
    }

    // Finish (✓)
    let finish_rect = Rect::from_center_size(egui::pos2(finish_x, button_y), Vec2::splat(28.0));
    let finish_resp = ui.allocate_rect(finish_rect, egui::Sense::click());
    if finish_resp.hovered() {
        ui.painter().circle_filled(finish_rect.center(), 14.0, Color32::from_rgb(60, 60, 65));
    }
    ui.painter().text(
        finish_rect.center(),
        egui::Align2::CENTER_CENTER,
        "✓",
        egui::FontId::proportional(16.0),
        Color32::from_rgb(100, 200, 100),
    );
    if finish_resp.clicked() {
        *on_finish = true;
    }
}

fn draw_processing(ui: &mut egui::Ui, rect: Rect, progress: Option<f32>) {
    ui.with_layout(egui::Layout::centered_and_justified(egui::Direction::LeftToRight), |ui| {
        if let Some(p) = progress {
            let bar_w = rect.width() * 0.6;
            let bar_rect = Rect::from_center_size(rect.center(), Vec2::new(bar_w, 6.0));
            ui.painter().rect_filled(bar_rect, Rounding::same(3.0), Color32::from_rgb(50, 50, 55));
            let fill_rect = Rect::from_min_size(bar_rect.min, Vec2::new(bar_w * p, 6.0));
            ui.painter().rect_filled(fill_rect, Rounding::same(3.0), accent());
        } else {
            ui.label(RichText::new("Polishing…").color(Color32::from_rgb(180, 180, 185)).size(14.0));
        }
    });
}

fn draw_done(ui: &mut egui::Ui, _rect: Rect, message: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(RichText::new(message).color(Color32::from_rgb(180, 180, 185)).size(13.0));
    });
}
