use egui::{Color32, Context, RichText, Ui, ViewportBuilder, ViewportId};

use crate::hotkey::HotkeyChoice;
use crate::polisher::CleanupLevel;
use crate::ui::accent;

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsTab {
    General,
    SpeechModel,
    Hotkey,
    Account,
    About,
}

impl Default for SettingsTab {
    fn default() -> Self {
        Self::General
    }
}

pub struct SettingsState {
    pub open: bool,
    pub tab: SettingsTab,
    pub cleanup_level: CleanupLevel,
    pub hotkey_choice: HotkeyChoice,
    pub hold_to_talk: bool,
    pub hands_free: bool,
    pub user_profile: String,
    pub input_device: Option<String>,
    pub available_devices: Vec<String>,
    pub signed_in: bool,
    pub user_email: String,
    pub app_version: String,
}

impl SettingsState {
    pub fn new(version: String) -> Self {
        Self {
            open: false,
            tab: SettingsTab::General,
            cleanup_level: CleanupLevel::Standard,
            hotkey_choice: HotkeyChoice::default(),
            hold_to_talk: true,
            hands_free: false,
            user_profile: String::new(),
            input_device: None,
            available_devices: Vec::new(),
            signed_in: false,
            user_email: String::new(),
            app_version: version,
        }
    }
}

pub enum SettingsEvent {
    CleanupLevelChanged(CleanupLevel),
    HotkeyChanged(HotkeyChoice),
    TriggerModeChanged { hold: bool, free: bool },
    UserProfileChanged(String),
    InputDeviceChanged(String),
    SignInRequested,
    SignOutRequested,
    Closed,
}

pub fn show(ctx: &Context, state: &mut SettingsState) -> Vec<SettingsEvent> {
    let mut events = Vec::new();

    if !state.open {
        return events;
    }

    ctx.show_viewport_immediate(
        ViewportId::from_hash_of("hush_settings"),
        ViewportBuilder::default()
            .with_title("Hush Settings")
            .with_inner_size([640.0, 480.0])
            .with_resizable(false),
        |ctx, _| {
            let evts = render_settings(ctx, state);
            events.extend(evts);
        },
    );

    events
}

fn render_settings(ctx: &Context, state: &mut SettingsState) -> Vec<SettingsEvent> {
    let mut events = Vec::new();

    egui::SidePanel::left("settings_sidebar")
        .exact_width(160.0)
        .resizable(false)
        .show(ctx, |ui| {
            ui.add_space(12.0);
            sidebar_item(ui, &mut state.tab, SettingsTab::General, "General");
            sidebar_item(ui, &mut state.tab, SettingsTab::SpeechModel, "Speech Model");
            sidebar_item(ui, &mut state.tab, SettingsTab::Hotkey, "Hotkey");
            sidebar_item(ui, &mut state.tab, SettingsTab::Account, "Account");
            sidebar_item(ui, &mut state.tab, SettingsTab::About, "About");
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.add_space(8.0);
        match state.tab {
            SettingsTab::General => events.extend(tab_general(ui, state)),
            SettingsTab::SpeechModel => tab_speech_model(ui, state),
            SettingsTab::Hotkey => events.extend(tab_hotkey(ui, state)),
            SettingsTab::Account => events.extend(tab_account(ui, state)),
            SettingsTab::About => tab_about(ui, state),
        }
    });

    if ctx.input(|i| i.viewport().close_requested()) {
        state.open = false;
        events.push(SettingsEvent::Closed);
    }

    events
}

fn sidebar_item(ui: &mut Ui, current: &mut SettingsTab, tab: SettingsTab, label: &str) {
    let selected = *current == tab;
    let text = if selected {
        RichText::new(label).color(accent()).strong()
    } else {
        RichText::new(label).color(Color32::from_rgb(180, 180, 185))
    };

    if ui
        .add(egui::Label::new(text).sense(egui::Sense::click()))
        .clicked()
    {
        *current = tab;
    }
    ui.add_space(4.0);
}

fn tab_general(ui: &mut Ui, state: &mut SettingsState) -> Vec<SettingsEvent> {
    let mut events = Vec::new();
    ui.heading("General");
    ui.add_space(12.0);

    ui.label("AI Cleanup Level");
    egui::ComboBox::from_id_source("cleanup_level")
        .selected_text(state.cleanup_level.label())
        .show_ui(ui, |ui| {
            for level in [
                CleanupLevel::Off,
                CleanupLevel::Light,
                CleanupLevel::Standard,
                CleanupLevel::Polished,
            ] {
                if ui
                    .selectable_value(&mut state.cleanup_level, level, level.label())
                    .clicked()
                {
                    events.push(SettingsEvent::CleanupLevelChanged(level));
                }
            }
        });

    ui.add_space(16.0);
    ui.label("About You (optional)");
    ui.small("Sent to the AI to personalise corrections — name, role, domain.");
    let resp = ui.text_edit_multiline(&mut state.user_profile);
    if resp.lost_focus() {
        events.push(SettingsEvent::UserProfileChanged(state.user_profile.clone()));
    }

    ui.add_space(16.0);
    ui.label("Microphone");
    let current_device = state
        .input_device
        .clone()
        .unwrap_or_else(|| "Default".to_string());
    egui::ComboBox::from_id_source("input_device")
        .selected_text(&current_device)
        .show_ui(ui, |ui| {
            if ui.selectable_label(state.input_device.is_none(), "Default").clicked() {
                state.input_device = None;
            }
            for device in &state.available_devices.clone() {
                if ui
                    .selectable_label(state.input_device.as_deref() == Some(device), device)
                    .clicked()
                {
                    state.input_device = Some(device.clone());
                    events.push(SettingsEvent::InputDeviceChanged(device.clone()));
                }
            }
        });

    events
}

fn tab_speech_model(ui: &mut Ui, _state: &mut SettingsState) {
    ui.heading("Speech Model");
    ui.add_space(12.0);
    ui.label(RichText::new("whisper.cpp base.en").strong());
    ui.small("On-device, runs fully offline. No audio sent to cloud.");
    ui.add_space(8.0);
    ui.label(RichText::new("Status: ").strong());
    ui.label("Model loaded ✓");
    ui.add_space(16.0);
    ui.small("Additional model sizes (small, medium) coming soon.");
}

fn tab_hotkey(ui: &mut Ui, state: &mut SettingsState) -> Vec<SettingsEvent> {
    let mut events = Vec::new();
    ui.heading("Hotkey");
    ui.add_space(12.0);

    ui.label("Dictation Key");
    egui::ComboBox::from_id_source("hotkey_choice")
        .selected_text(state.hotkey_choice.label())
        .show_ui(ui, |ui| {
            for choice in [
                HotkeyChoice::RightAlt,
                HotkeyChoice::RightCtrl,
                HotkeyChoice::RightShift,
                HotkeyChoice::CapsLock,
                HotkeyChoice::F13,
                HotkeyChoice::F14,
            ] {
                if ui
                    .selectable_value(&mut state.hotkey_choice, choice, choice.label())
                    .clicked()
                {
                    events.push(SettingsEvent::HotkeyChanged(choice));
                }
            }
        });

    ui.add_space(16.0);
    ui.label("Trigger Mode");
    let mut changed = false;
    changed |= ui.checkbox(&mut state.hold_to_talk, "Hold to talk").changed();
    changed |= ui.checkbox(&mut state.hands_free, "Double-tap for hands-free").changed();
    if changed {
        events.push(SettingsEvent::TriggerModeChanged {
            hold: state.hold_to_talk,
            free: state.hands_free,
        });
    }

    events
}

fn tab_account(ui: &mut Ui, state: &mut SettingsState) -> Vec<SettingsEvent> {
    let mut events = Vec::new();
    ui.heading("Account");
    ui.add_space(12.0);

    if state.signed_in {
        ui.label(format!("Signed in as {}", state.user_email));
        ui.add_space(8.0);
        if ui.button("Sign Out").clicked() {
            events.push(SettingsEvent::SignOutRequested);
        }
    } else {
        ui.label("Sign in to enable AI cleanup (Stage 2 polish).");
        ui.small("Your audio is never sent to any cloud without your permission.");
        ui.add_space(8.0);
        if ui.button("Sign In via Browser").clicked() {
            events.push(SettingsEvent::SignInRequested);
        }
    }

    events
}

fn tab_about(ui: &mut Ui, state: &SettingsState) {
    ui.heading("Hush for Windows");
    ui.add_space(8.0);
    ui.label(RichText::new(format!("Version {}", state.app_version)).weak());
    ui.add_space(12.0);
    ui.label("Fast, private, system-wide voice dictation.");
    ui.small("Stage 1 — Whisper base.en (on-device, offline)");
    ui.small("Stage 2 — Groq llama-3.1-8b-instant (optional, cloud)");
}
