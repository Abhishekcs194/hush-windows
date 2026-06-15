pub mod bubble;
pub mod settings;
pub mod tray;

pub use bubble::BubbleState;

/// Accent color — matches the macOS app's brand pink
pub fn accent() -> egui::Color32 {
    egui::Color32::from_rgb(247, 69, 161)
}

pub fn accent_dim() -> egui::Color32 {
    egui::Color32::from_rgba_premultiplied(247, 69, 161, 120)
}
