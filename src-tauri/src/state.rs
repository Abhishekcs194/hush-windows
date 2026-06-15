use std::sync::{Arc, Mutex};

use crate::auth::AuthStore;
use crate::history::History;
use crate::hotkey::HotkeyChoice;
use crate::polisher::CleanupLevel;
use crate::transcriber::{TranscriptionSegment, Transcriber};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub cleanup_level: CleanupLevel,
    pub hotkey_choice: HotkeyChoice,
    pub hold_to_talk: bool,
    pub hands_free: bool,
    pub user_profile: String,
    pub input_device: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            cleanup_level: CleanupLevel::Standard,
            hotkey_choice: HotkeyChoice::default(),
            hold_to_talk: true,
            hands_free: false,
            user_profile: String::new(),
            input_device: None,
        }
    }
}

impl Settings {
    pub fn load() -> Self {
        let path = Self::path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    fn path() -> std::path::PathBuf {
        dirs::data_dir()
            .unwrap_or_default()
            .join("Hush")
            .join("settings.json")
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum RecordingState {
    Idle,
    Recording,
    Processing,
}

pub struct AppState {
    pub auth: Arc<AuthStore>,
    pub history: Arc<Mutex<History>>,
    pub settings: Arc<Mutex<Settings>>,
    pub transcriber: Arc<tokio::sync::Mutex<Option<Transcriber>>>,
    pub recording_state: Arc<Mutex<RecordingState>>,
    pub pending_segments: Arc<Mutex<Vec<TranscriptionSegment>>>,
    pub rt: tokio::runtime::Handle,
}

impl AppState {
    pub fn new(rt: tokio::runtime::Handle) -> Self {
        Self {
            auth: Arc::new(AuthStore::new()),
            history: Arc::new(Mutex::new(History::load())),
            settings: Arc::new(Mutex::new(Settings::load())),
            transcriber: Arc::new(tokio::sync::Mutex::new(None)),
            recording_state: Arc::new(Mutex::new(RecordingState::Idle)),
            pending_segments: Arc::new(Mutex::new(Vec::new())),
            rt,
        }
    }
}
