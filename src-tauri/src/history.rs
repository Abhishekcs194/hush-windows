use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

const MAX_ENTRIES: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub text: String,
    pub raw_text: Option<String>,
    pub app_name: String,
    pub timestamp: DateTime<Utc>,
}

impl HistoryEntry {
    pub fn new(text: String, raw_text: Option<String>, app_name: String) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            text,
            raw_text,
            app_name,
            timestamp: Utc::now(),
        }
    }
}

pub struct History {
    entries: Vec<HistoryEntry>,
    path: PathBuf,
}

impl History {
    pub fn load() -> Self {
        let path = Self::history_path();
        let entries = Self::read_from_disk(&path).unwrap_or_default();
        Self { entries, path }
    }

    pub fn push(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_ENTRIES);
        let _ = self.save_to_disk();
    }

    pub fn entries(&self) -> &[HistoryEntry] {
        &self.entries
    }

    pub fn recent_for_app(&self, app_name: &str, limit: usize) -> Vec<&HistoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.app_name == app_name)
            .take(limit)
            .collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        let _ = self.save_to_disk();
    }

    fn history_path() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Hush")
            .join("history.json")
    }

    fn read_from_disk(path: &PathBuf) -> Option<Vec<HistoryEntry>> {
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    fn save_to_disk(&self) -> anyhow::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&self.path, data)?;
        Ok(())
    }
}
