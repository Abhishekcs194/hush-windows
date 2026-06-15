use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIcon, TrayIconBuilder,
};

pub enum TrayAction {
    ShowSettings,
    ShowHistory,
    SignIn,
    SignOut,
    Quit,
}

pub struct Tray {
    _icon: TrayIcon,
    settings_item_id: tray_icon::menu::MenuId,
    history_item_id: tray_icon::menu::MenuId,
    sign_in_item_id: tray_icon::menu::MenuId,
    sign_out_item_id: tray_icon::menu::MenuId,
    quit_item_id: tray_icon::menu::MenuId,
}

impl Tray {
    pub fn new(signed_in: bool) -> anyhow::Result<Self> {
        let menu = Menu::new();

        let settings_item = MenuItem::new("Settings", true, None);
        let history_item = MenuItem::new("History", true, None);
        let sign_in_item = MenuItem::new("Sign In", !signed_in, None);
        let sign_out_item = MenuItem::new("Sign Out", signed_in, None);
        let quit_item = MenuItem::new("Quit Hush", true, None);

        let settings_id = settings_item.id().clone();
        let history_id = history_item.id().clone();
        let sign_in_id = sign_in_item.id().clone();
        let sign_out_id = sign_out_item.id().clone();
        let quit_id = quit_item.id().clone();

        menu.append_items(&[
            &settings_item,
            &history_item,
            &PredefinedMenuItem::separator(),
            &sign_in_item,
            &sign_out_item,
            &PredefinedMenuItem::separator(),
            &quit_item,
        ])?;

        let icon = make_icon();
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("Hush — Voice Dictation")
            .with_icon(icon)
            .build()?;

        Ok(Self {
            _icon: tray,
            settings_item_id: settings_id,
            history_item_id: history_id,
            sign_in_item_id: sign_in_id,
            sign_out_item_id: sign_out_id,
            quit_item_id: quit_id,
        })
    }

    /// Poll for tray menu events. Call every frame.
    pub fn poll(&self) -> Option<TrayAction> {
        if let Ok(event) = MenuEvent::receiver().try_recv() {
            let id = &event.id;
            if id == &self.settings_item_id {
                return Some(TrayAction::ShowSettings);
            }
            if id == &self.history_item_id {
                return Some(TrayAction::ShowHistory);
            }
            if id == &self.sign_in_item_id {
                return Some(TrayAction::SignIn);
            }
            if id == &self.sign_out_item_id {
                return Some(TrayAction::SignOut);
            }
            if id == &self.quit_item_id {
                return Some(TrayAction::Quit);
            }
        }
        None
    }
}

/// Generate a simple 16x16 pink waveform icon from raw RGBA bytes
fn make_icon() -> tray_icon::Icon {
    const W: u32 = 16;
    const H: u32 = 16;
    let mut rgba = vec![0u8; (W * H * 4) as usize];

    // Draw three vertical bars in the center to suggest a waveform
    let bars = [(4u32, 6u32), (7, 10), (10, 6), (13, 8)];
    for (x, bar_h) in bars {
        let y_start = (H - bar_h) / 2;
        for y in y_start..(y_start + bar_h) {
            let idx = ((y * W + x) * 4) as usize;
            rgba[idx] = 247;     // R
            rgba[idx + 1] = 69;  // G
            rgba[idx + 2] = 161; // B
            rgba[idx + 3] = 255; // A
        }
    }

    tray_icon::Icon::from_rgba(rgba, W, H).expect("Failed to create tray icon")
}
