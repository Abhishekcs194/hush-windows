import { invoke } from "@tauri-apps/api/core";

export interface Settings {
  cleanup_level: "Off" | "Light" | "Standard" | "Polished";
  hotkey_choice: "RightAlt" | "RightCtrl" | "RightShift" | "CapsLock" | "F13" | "F14";
  hold_to_talk: boolean;
  hands_free: boolean;
  user_profile: string;
  input_device: string | null;
}

export interface AuthInfo {
  signed_in: boolean;
  pair_url: string;
}

export interface HistoryEntry {
  id: string;
  text: string;
  raw_text: string | null;
  app_name: string;
  timestamp: string;
}

export const getSettings = (): Promise<Settings> =>
  invoke("get_settings");

export const saveSettings = (settings: Settings): Promise<void> =>
  invoke("save_settings", { settings });

export const getHistory = (): Promise<HistoryEntry[]> =>
  invoke("get_history");

export const clearHistory = (): Promise<void> =>
  invoke("clear_history");

export const getAuthState = (): Promise<AuthInfo> =>
  invoke("get_auth_state");

export const signOut = (): Promise<void> =>
  invoke("sign_out");

export const openSignIn = (): Promise<void> =>
  invoke("open_sign_in");

export const getAudioDevices = (): Promise<string[]> =>
  invoke("get_audio_devices");

export const getTranscriberReady = (): Promise<boolean> =>
  invoke("get_transcriber_ready");
