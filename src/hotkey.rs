use global_hotkey::{
    hotkey::{Code, HotKey, Modifiers},
    GlobalHotKeyEvent, GlobalHotKeyManager,
};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HotkeyChoice {
    RightAlt,
    RightCtrl,
    RightShift,
    CapsLock,
    F13,
    F14,
}

impl HotkeyChoice {
    pub fn label(&self) -> &'static str {
        match self {
            Self::RightAlt => "Right Alt",
            Self::RightCtrl => "Right Ctrl",
            Self::RightShift => "Right Shift",
            Self::CapsLock => "Caps Lock",
            Self::F13 => "F13",
            Self::F14 => "F14",
        }
    }

    fn to_code(&self) -> Code {
        match self {
            Self::RightAlt => Code::AltRight,
            Self::RightCtrl => Code::ControlRight,
            Self::RightShift => Code::ShiftRight,
            Self::CapsLock => Code::CapsLock,
            Self::F13 => Code::F13,
            Self::F14 => Code::F14,
        }
    }
}

impl Default for HotkeyChoice {
    fn default() -> Self {
        Self::RightAlt
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HotkeyEvent {
    /// User pressed and held — start recording
    HoldStart,
    /// User released after hold — stop recording
    HoldEnd,
    /// User double-tapped — toggle hands-free mode
    DoubleTap,
}

pub struct HotkeyManager {
    manager: GlobalHotKeyManager,
    hotkey: HotKey,
    hotkey_id: u32,
    choice: HotkeyChoice,
    last_press: Option<Instant>,
    press_count: u32,
    is_held: bool,
}

impl HotkeyManager {
    pub fn new(choice: HotkeyChoice) -> anyhow::Result<Self> {
        let manager = GlobalHotKeyManager::new()?;
        let hotkey = HotKey::new(None, choice.to_code());
        let hotkey_id = hotkey.id();
        manager.register(hotkey)?;
        Ok(Self {
            manager,
            hotkey,
            hotkey_id,
            choice,
            last_press: None,
            press_count: 0,
            is_held: false,
        })
    }

    /// Call this every frame from the egui update loop.
    /// Returns Some(HotkeyEvent) if something actionable happened.
    pub fn poll(&mut self) -> Option<HotkeyEvent> {
        use global_hotkey::HotKeyState;

        if let Ok(event) = GlobalHotKeyEvent::receiver().try_recv() {
            if event.id != self.hotkey_id {
                return None;
            }

            match event.state {
                HotKeyState::Pressed => {
                    let now = Instant::now();
                    let is_double = self
                        .last_press
                        .map(|t| now.duration_since(t) < DOUBLE_TAP_WINDOW)
                        .unwrap_or(false);

                    self.last_press = Some(now);
                    self.press_count += 1;
                    self.is_held = true;

                    if is_double {
                        self.press_count = 0;
                        self.last_press = None;
                        return Some(HotkeyEvent::DoubleTap);
                    }

                    return Some(HotkeyEvent::HoldStart);
                }
                HotKeyState::Released => {
                    self.is_held = false;
                    return Some(HotkeyEvent::HoldEnd);
                }
            }
        }
        None
    }

    pub fn reconfigure(&mut self, choice: HotkeyChoice) -> anyhow::Result<()> {
        let _ = self.manager.unregister(self.hotkey);
        let hotkey = HotKey::new(None, choice.to_code());
        self.hotkey_id = hotkey.id();
        self.hotkey = hotkey;
        self.choice = choice;
        self.manager.register(self.hotkey)?;
        Ok(())
    }

    pub fn choice(&self) -> HotkeyChoice {
        self.choice
    }
}
