mod audio;
mod auth;
mod commands;
mod context;
mod history;
mod hotkey;
mod injector;
mod polisher;
mod state;
mod transcriber;

use std::sync::{Arc, Mutex};

use audio::{AudioCapture, AudioEvent};
use crossbeam_channel::bounded;
use hotkey::{HotkeyEvent, HotkeyManager};
use injector::{InjectionResult, TextInjector};
use polisher::Polisher;
use state::{AppState, RecordingState};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager,
};
use transcriber::Transcriber;

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to build Tokio runtime");

    let rt_handle = runtime.handle().clone();
    let app_state = AppState::new(rt_handle.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        // Keep tokio runtime alive as a managed resource
        .manage(runtime)
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::save_settings,
            commands::get_history,
            commands::clear_history,
            commands::get_auth_state,
            commands::sign_out,
            commands::open_sign_in,
            commands::get_audio_devices,
            commands::get_transcriber_ready,
        ])
        .setup(|app| {
            setup_app(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Error running Hush");
}

fn setup_app(app: &mut tauri::App) -> anyhow::Result<()> {
    let handle = app.handle().clone();

    // ── System tray ────────────────────────────────────────────────────────
    let settings_item = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let history_item = MenuItem::with_id(app, "history", "History", true, None::<&str>)?;
    let signin_item = MenuItem::with_id(app, "signin", "Sign In", true, None::<&str>)?;
    let signout_item = MenuItem::with_id(app, "signout", "Sign Out", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "Quit Hush", true, None::<&str>)?;
    let separator = tauri::menu::PredefinedMenuItem::separator(app)?;

    let menu = Menu::with_items(
        app,
        &[
            &settings_item,
            &history_item,
            &separator,
            &signin_item,
            &signout_item,
            &tauri::menu::PredefinedMenuItem::separator(app)?,
            &quit_item,
        ],
    )?;

    let _tray = TrayIconBuilder::new()
        .icon(make_tray_icon())
        .menu(&menu)
        .tooltip("Hush — Voice Dictation")
        .on_menu_event({
            let handle = handle.clone();
            move |app, event| match event.id.as_ref() {
                "settings" => open_settings_window(app),
                "history" => open_history_window(app),
                "signin" => { let _ = open::that(auth::pair_url()); }
                "signout" => {
                    let state = app.state::<AppState>();
                    state.auth.sign_out();
                    let _ = app.emit("auth-changed", false);
                }
                "quit" => app.exit(0),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                open_settings_window(tray.app_handle());
            }
        })
        .build(app)?;

    // ── Load Whisper model in background ───────────────────────────────────
    {
        let state = app.state::<AppState>();
        let tr = Arc::clone(&state.transcriber);
        let emit_handle = handle.clone();
        state.rt.spawn(async move {
            match Transcriber::new().await {
                Ok(t) => {
                    *tr.lock().await = Some(t);
                    log::info!("Transcriber ready");
                    let _ = emit_handle.emit("transcriber-ready", ());
                }
                Err(e) => log::error!("Failed to load Whisper model: {}", e),
            }
        });
    }

    // ── Hotkey listener ────────────────────────────────────────────────────
    {
        let state = app.state::<AppState>();
        let choice = state.settings.lock().unwrap().hotkey_choice;
        drop(state); // release borrow before moving handle

        let app_handle = handle.clone();
        std::thread::spawn(move || {
            let mut mgr = match HotkeyManager::new(choice) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Failed to register hotkey: {}", e);
                    return;
                }
            };

            let mut last_press: Option<std::time::Instant> = None;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(5));
                match mgr.poll() {
                    Some(HotkeyEvent::HoldStart) => {
                        let now = std::time::Instant::now();
                        let is_double = last_press
                            .map(|t| now.duration_since(t) < std::time::Duration::from_millis(300))
                            .unwrap_or(false);
                        last_press = Some(now);

                        if is_double {
                            let _ = app_handle.emit("hotkey-double-tap", ());
                        } else {
                            let _ = app_handle.emit("hotkey-down", ());
                        }
                    }
                    Some(HotkeyEvent::HoldEnd) => {
                        let _ = app_handle.emit("hotkey-up", ());
                    }
                    Some(HotkeyEvent::DoubleTap) => {
                        let _ = app_handle.emit("hotkey-double-tap", ());
                    }
                    None => {}
                }
            }
        });
    }

    // ── Audio event bridge ─────────────────────────────────────────────────
    // Listens to Tauri events from the frontend (start/stop recording requests)
    // and manages the audio capture + transcription pipeline.
    {
        let app_handle = handle.clone();
        let state_ref = app.state::<AppState>();
        let auth = Arc::clone(&state_ref.auth);
        let history = Arc::clone(&state_ref.history);
        let settings = Arc::clone(&state_ref.settings);
        let transcriber = Arc::clone(&state_ref.transcriber);
        let recording_state = Arc::clone(&state_ref.recording_state);
        let pending_segments = Arc::clone(&state_ref.pending_segments);
        let rt = state_ref.rt.clone();

        let (audio_tx, audio_rx) = bounded::<AudioEvent>(256);

        app.listen("start-recording", {
            let audio_tx = audio_tx.clone();
            let recording_state = Arc::clone(&recording_state);
            let app_handle = app_handle.clone();
            move |_| {
                let mut rs = recording_state.lock().unwrap();
                if *rs != RecordingState::Idle {
                    return;
                }
                *rs = RecordingState::Recording;
                drop(rs);

                let mut capture = match AudioCapture::new(audio_tx.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        log::error!("Audio capture error: {}", e);
                        return;
                    }
                };
                if let Err(e) = capture.start() {
                    log::error!("Audio start error: {}", e);
                    return;
                }
                // Store capture handle — for stop, we use a separate event
                // (simplified: capture dropped when stop-recording fires)
                let _ = app_handle.emit("recording-started", ());
            }
        });

        // Audio event → Tauri event bridge (background thread)
        let bridge_handle = app_handle.clone();
        let bridge_transcriber = Arc::clone(&transcriber);
        let bridge_auth = Arc::clone(&auth);
        let bridge_history = Arc::clone(&history);
        let bridge_settings = Arc::clone(&settings);
        let bridge_segments = Arc::clone(&pending_segments);
        let bridge_recording = Arc::clone(&recording_state);
        let bridge_rt = rt.clone();

        std::thread::spawn(move || {
            loop {
                match audio_rx.recv() {
                    Ok(AudioEvent::LevelUpdate(rms)) => {
                        let _ = bridge_handle.emit("level-update", rms);
                    }
                    Ok(AudioEvent::RecordingStarted) => {}
                    Ok(AudioEvent::RecordingStopped) => {
                        *bridge_recording.lock().unwrap() = RecordingState::Processing;
                        let _ = bridge_handle.emit("processing-started", ());

                        // Run polish + inject
                        let segments = bridge_segments.lock().unwrap().clone();
                        if segments.is_empty() {
                            *bridge_recording.lock().unwrap() = RecordingState::Idle;
                            let _ = bridge_handle.emit("recording-cancelled", ());
                            continue;
                        }

                        let tr = Arc::clone(&bridge_transcriber);
                        let auth = Arc::clone(&bridge_auth);
                        let history = Arc::clone(&bridge_history);
                        let settings_snap = bridge_settings.lock().unwrap().clone();
                        let emit = bridge_handle.clone();
                        let rec_state = Arc::clone(&bridge_recording);
                        let segs_ref = Arc::clone(&bridge_segments);

                        bridge_rt.spawn(async move {
                            // Transcribe pending audio chunks
                            // (chunks arrive via AudioEvent::Chunk handled below)
                            let final_segments = segs_ref.lock().unwrap().clone();

                            // Stage 2 polish
                            let app_ctx = context::capture();
                            let recent: Vec<_> = {
                                let h = history.lock().unwrap();
                                h.recent_for_app(&app_ctx.process_name, 3)
                                    .into_iter()
                                    .cloned()
                                    .collect()
                            };

                            let mut polisher = Polisher::new(auth.token());
                            let text = polisher
                                .polish(
                                    &final_segments,
                                    settings_snap.cleanup_level,
                                    app_ctx.category,
                                    Some(&settings_snap.user_profile),
                                    &recent,
                                )
                                .await;

                            // Inject text
                            let mut injector = injector::TextInjector::new();
                            let result = injector.inject(&text);
                            let injected = matches!(result, InjectionResult::Success(_));
                            let message = if injected {
                                "Inserted ✓".to_string()
                            } else {
                                "Copied — press Ctrl+V".to_string()
                            };

                            // Save to history
                            let raw = final_segments
                                .iter()
                                .map(|s| s.text.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            let entry = history::HistoryEntry::new(
                                text.clone(),
                                Some(raw),
                                app_ctx.process_name,
                            );
                            history.lock().unwrap().push(entry);
                            segs_ref.lock().unwrap().clear();

                            *rec_state.lock().unwrap() = RecordingState::Idle;
                            let _ = emit.emit(
                                "dictation-complete",
                                serde_json::json!({ "text": text, "message": message, "injected": injected }),
                            );
                        });
                    }
                    Ok(AudioEvent::Chunk(chunk)) => {
                        // Transcribe chunk
                        let tr = Arc::clone(&bridge_transcriber);
                        let segs = Arc::clone(&bridge_segments);
                        let pause = chunk.pause_before_secs;
                        bridge_rt.spawn(async move {
                            let guard = tr.lock().await;
                            if let Some(t) = guard.as_ref() {
                                match t.transcribe(&chunk.samples, chunk.sample_rate, pause).await {
                                    Ok(seg) if !seg.text.is_empty() => {
                                        segs.lock().unwrap().push(seg);
                                    }
                                    Err(e) => log::warn!("Transcription error: {}", e),
                                    _ => {}
                                }
                            }
                        });
                    }
                    Err(_) => break,
                }
            }
        });
    }

    // ── Create bubble window (hidden, shown when recording starts) ─────────
    create_bubble_window(app)?;

    Ok(())
}

fn create_bubble_window(app: &tauri::App) -> anyhow::Result<()> {
    let monitor = app
        .primary_monitor()?
        .ok_or_else(|| anyhow::anyhow!("No primary monitor"))?;
    let w = monitor.size().width as f64 / monitor.scale_factor();
    let h = monitor.size().height as f64 / monitor.scale_factor();

    tauri::WebviewWindowBuilder::new(app, "bubble", tauri::WebviewUrl::App("index.html#bubble".into()))
        .title("Hush")
        .inner_size(420.0, 80.0)
        .position(w / 2.0 - 210.0, h - 110.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .build()?;

    Ok(())
}

fn open_settings_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("settings") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "settings",
        tauri::WebviewUrl::App("index.html#settings".into()),
    )
    .title("Hush Settings")
    .inner_size(680.0, 480.0)
    .resizable(false)
    .build();
}

fn open_history_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("history") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "history",
        tauri::WebviewUrl::App("index.html#history".into()),
    )
    .title("Hush History")
    .inner_size(560.0, 600.0)
    .build();
}

fn make_tray_icon() -> Image<'static> {
    const W: u32 = 16;
    const H: u32 = 16;
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for (x, bar_h) in [(3u32, 6u32), (6, 10), (9, 7), (12, 9)] {
        let y_start = (H - bar_h) / 2;
        for y in y_start..(y_start + bar_h) {
            let idx = ((y * W + x) * 4) as usize;
            rgba[idx] = 247;
            rgba[idx + 1] = 69;
            rgba[idx + 2] = 161;
            rgba[idx + 3] = 255;
        }
    }
    Image::new_owned(rgba, W, H)
}
