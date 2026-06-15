use serde::{Deserialize, Serialize};

#[cfg(windows)]
use windows::Win32::{
    Foundation::HWND,
    System::ProcessStatus::GetModuleFileNameExW,
    System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ},
    UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowThreadProcessId},
};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum AppCategory {
    Coding,
    Terminal,
    Writing,
    Messaging,
    Email,
    General,
}

impl AppCategory {
    pub fn format_hint(&self) -> &'static str {
        match self {
            Self::Coding => "Coding/IDE: preserve identifiers, keep API/JSON/variable names exact. No auto-capitalise.",
            Self::Terminal => "Terminal: preserve commands, flags, paths. No punctuation added.",
            Self::Writing => "Writing/docs: full punctuation, paragraph breaks on <break>.",
            Self::Messaging => "Messaging: conversational, light punctuation, no formal salutation.",
            Self::Email => "Email: professional tone, full punctuation, greeting/sign-off if implied.",
            Self::General => "General: natural punctuation, preserve speaker intent.",
        }
    }

    fn from_process_name(name: &str) -> Self {
        let n = name.to_lowercase();
        if n.contains("code") || n.contains("rider") || n.contains("idea") || n.contains("clion")
            || n.contains("pycharm") || n.contains("webstorm") || n.contains("cursor")
            || n.contains("fleet") || n.contains("studio")
        {
            return Self::Coding;
        }
        if n.contains("terminal") || n.contains("cmd") || n.contains("powershell")
            || n.contains("wt") || n.contains("conhost") || n.contains("mintty")
            || n.contains("bash") || n.contains("hyper")
        {
            return Self::Terminal;
        }
        if n.contains("slack") || n.contains("discord") || n.contains("teams")
            || n.contains("telegram") || n.contains("whatsapp") || n.contains("signal")
            || n.contains("messenger") || n.contains("chat")
        {
            return Self::Messaging;
        }
        if n.contains("outlook") || n.contains("thunderbird") || n.contains("mail") {
            return Self::Email;
        }
        if n.contains("word") || n.contains("docs") || n.contains("notion")
            || n.contains("obsidian") || n.contains("typora") || n.contains("notepad")
            || n.contains("writer")
        {
            return Self::Writing;
        }
        Self::General
    }
}

#[derive(Debug, Clone)]
pub struct AppContext {
    pub process_name: String,
    pub window_title: String,
    pub category: AppCategory,
}

impl AppContext {
    pub fn unknown() -> Self {
        Self {
            process_name: String::new(),
            window_title: String::new(),
            category: AppCategory::General,
        }
    }
}

pub fn capture() -> AppContext {
    #[cfg(windows)]
    {
        capture_windows()
    }
    #[cfg(not(windows))]
    {
        AppContext::unknown()
    }
}

#[cfg(windows)]
fn capture_windows() -> AppContext {
    unsafe {
        let hwnd: HWND = GetForegroundWindow();
        if hwnd.0 == 0 {
            return AppContext::unknown();
        }

        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 {
            return AppContext::unknown();
        }

        let handle = match OpenProcess(
            PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,
            false,
            pid,
        ) {
            Ok(h) => h,
            Err(_) => return AppContext::unknown(),
        };

        let mut buf = vec![0u16; 260];
        let len = GetModuleFileNameExW(handle, None, &mut buf) as usize;
        let _ = windows::Win32::Foundation::CloseHandle(handle);

        let full_path = String::from_utf16_lossy(&buf[..len]);
        let process_name = std::path::Path::new(&full_path)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();

        let category = AppCategory::from_process_name(&process_name);

        AppContext {
            process_name,
            window_title: String::new(), // Can be added via GetWindowTextW if needed
            category,
        }
    }
}
