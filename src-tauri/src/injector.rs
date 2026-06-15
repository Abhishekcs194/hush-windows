use arboard::Clipboard;
use std::time::Duration;

#[cfg(windows)]
use windows::Win32::{
    Foundation::HWND,
    UI::{
        Input::KeyboardAndMouse::{
            SendInput, INPUT, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
            VK_CONTROL, VK_V,
        },
        WindowsAndMessaging::{GetForegroundWindow, SetForegroundWindow},
    },
};

#[derive(Debug, Clone)]
pub enum InjectionMethod {
    Clipboard,
    DirectType,
}

#[derive(Debug, Clone)]
pub enum InjectionResult {
    Success(InjectionMethod),
    Copied, // Text is on clipboard, user should press Ctrl+V
}

pub struct TextInjector {
    prior_clipboard: Option<String>,
}

impl TextInjector {
    pub fn new() -> Self {
        Self {
            prior_clipboard: None,
        }
    }

    pub fn inject(&mut self, text: &str) -> InjectionResult {
        // 1. Save user's clipboard
        self.prior_clipboard = read_clipboard();

        // 2. Put our text on the clipboard
        if !write_clipboard(text) {
            log::warn!("Failed to write to clipboard");
            return InjectionResult::Copied;
        }

        // Small settle delay so the target app sees the clipboard update
        std::thread::sleep(Duration::from_millis(30));

        // 3. Send Ctrl+V to the focused window
        #[cfg(windows)]
        {
            if send_ctrl_v() {
                // Schedule clipboard restore after paste has time to land
                let prior = self.prior_clipboard.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(250));
                    if let Some(original) = prior {
                        write_clipboard(&original);
                    }
                });
                return InjectionResult::Success(InjectionMethod::Clipboard);
            }
        }

        InjectionResult::Copied
    }

    /// Call if the user manually pressed Ctrl+V after receiving InjectionResult::Copied
    pub fn on_manual_paste(&mut self) {
        if let Some(original) = self.prior_clipboard.take() {
            std::thread::sleep(Duration::from_millis(100));
            write_clipboard(&original);
        }
    }

    pub fn restore_clipboard(&mut self) {
        if let Some(original) = self.prior_clipboard.take() {
            write_clipboard(&original);
        }
    }
}

fn read_clipboard() -> Option<String> {
    Clipboard::new().ok()?.get_text().ok()
}

fn write_clipboard(text: &str) -> bool {
    match Clipboard::new() {
        Ok(mut cb) => cb.set_text(text).is_ok(),
        Err(_) => false,
    }
}

#[cfg(windows)]
fn send_ctrl_v() -> bool {
    unsafe {
        // Press Ctrl
        let ctrl_down = make_key_input(VK_CONTROL.0 as u16, false);
        // Press V
        let v_down = make_key_input(VK_V.0 as u16, false);
        // Release V
        let v_up = make_key_input(VK_V.0 as u16, true);
        // Release Ctrl
        let ctrl_up = make_key_input(VK_CONTROL.0 as u16, true);

        let inputs = [ctrl_down, v_down, v_up, ctrl_up];
        let sent = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        sent == inputs.len() as u32
    }
}

#[cfg(windows)]
fn make_key_input(vk: u16, key_up: bool) -> INPUT {
    let flags = if key_up {
        windows::Win32::UI::Input::KeyboardAndMouse::KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 {
            ki: KEYBDINPUT {
                wVk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY(vk),
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

impl Default for TextInjector {
    fn default() -> Self {
        Self::new()
    }
}
