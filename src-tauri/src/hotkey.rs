use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VIRTUAL_KEY, VK_CAPITAL, VK_CONTROL, VK_F13, VK_F14,
    VK_LWIN, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_RWIN, VK_SPACE,
};

const DOUBLE_TAP_WINDOW: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum HotkeyChoice {
    CtrlSpace,
    WinSpace,
    RightAlt,
    RightCtrl,
    RightShift,
    CapsLock,
    F13,
    F14,
}

impl HotkeyChoice {
    pub fn label(self) -> &'static str {
        match self {
            Self::CtrlSpace  => "Ctrl + Space",
            Self::WinSpace   => "Win + Space",
            Self::RightAlt   => "Right Alt",
            Self::RightCtrl  => "Right Ctrl",
            Self::RightShift => "Right Shift",
            Self::CapsLock   => "Caps Lock",
            Self::F13        => "F13",
            Self::F14        => "F14",
        }
    }

    fn is_down(self) -> bool {
        fn key(vk: VIRTUAL_KEY) -> bool {
            unsafe { GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000 != 0 }
        }
        match self {
            Self::CtrlSpace  => key(VK_CONTROL) && key(VK_SPACE),
            Self::WinSpace   => (key(VK_LWIN) || key(VK_RWIN)) && key(VK_SPACE),
            Self::RightAlt   => key(VK_RMENU),
            Self::RightCtrl  => key(VK_RCONTROL),
            Self::RightShift => key(VK_RSHIFT),
            Self::CapsLock   => key(VK_CAPITAL),
            Self::F13        => key(VK_F13),
            Self::F14        => key(VK_F14),
        }
    }
}

impl Default for HotkeyChoice {
    fn default() -> Self {
        Self::CtrlSpace
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HotkeyEvent {
    /// Combo pressed down — start recording
    HoldStart,
    /// Combo released — stop recording
    HoldEnd,
    /// Double-tapped within 300ms — toggle hands-free
    DoubleTap,
}

pub struct HotkeyManager {
    choice: HotkeyChoice,
    last_press: Option<Instant>,
    was_pressed: bool,
}

impl HotkeyManager {
    pub fn new(choice: HotkeyChoice) -> anyhow::Result<Self> {
        Ok(Self { choice, last_press: None, was_pressed: false })
    }

    /// Poll key state. Call every 5–10ms from a background thread.
    pub fn poll(&mut self) -> Option<HotkeyEvent> {
        let is_down = self.choice.is_down();

        if is_down && !self.was_pressed {
            self.was_pressed = true;
            let now = Instant::now();
            let is_double = self
                .last_press
                .map(|t| now.duration_since(t) < DOUBLE_TAP_WINDOW)
                .unwrap_or(false);
            self.last_press = Some(now);
            if is_double {
                self.last_press = None;
                return Some(HotkeyEvent::DoubleTap);
            }
            return Some(HotkeyEvent::HoldStart);
        }

        if !is_down && self.was_pressed {
            self.was_pressed = false;
            return Some(HotkeyEvent::HoldEnd);
        }

        None
    }

    pub fn reconfigure(&mut self, choice: HotkeyChoice) {
        self.choice = choice;
        self.was_pressed = false;
    }

    pub fn choice(&self) -> HotkeyChoice {
        self.choice
    }
}
