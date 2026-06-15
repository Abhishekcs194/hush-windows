use tauri::State;

use crate::audio::AudioCapture;
use crate::auth::pair_url;
use crate::history::HistoryEntry;
use crate::state::{AppState, Settings};

// ── Settings ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<Settings, String> {
    Ok(state.settings.lock().unwrap().clone())
}

#[tauri::command]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<(), String> {
    let mut s = state.settings.lock().unwrap();
    *s = settings;
    s.save().map_err(|e| e.to_string())
}

// ── History ─────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_history(state: State<'_, AppState>) -> Result<Vec<HistoryEntry>, String> {
    Ok(state.history.lock().unwrap().entries().to_vec())
}

#[tauri::command]
pub fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.history.lock().unwrap().clear();
    Ok(())
}

// ── Auth ─────────────────────────────────────────────────────────────────────

#[derive(serde::Serialize)]
pub struct AuthInfo {
    pub signed_in: bool,
    pub pair_url: String,
}

#[tauri::command]
pub fn get_auth_state(state: State<'_, AppState>) -> Result<AuthInfo, String> {
    Ok(AuthInfo {
        signed_in: state.auth.is_signed_in(),
        pair_url: pair_url(),
    })
}

#[tauri::command]
pub fn sign_out(state: State<'_, AppState>) -> Result<(), String> {
    state.auth.sign_out();
    Ok(())
}

#[tauri::command]
pub fn open_sign_in() -> Result<(), String> {
    open::that(pair_url()).map_err(|e| e.to_string())
}

// ── Devices ──────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_audio_devices() -> Result<Vec<String>, String> {
    AudioCapture::list_devices().map_err(|e| e.to_string())
}

// ── Transcriber status ───────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_transcriber_ready(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.transcriber.lock().await.is_some())
}
