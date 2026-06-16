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

use std::sync::{atomic::{AtomicI32, Ordering}, Arc};

use audio::{AudioCapture, AudioEvent};
use crossbeam_channel::bounded;
use hotkey::{HotkeyEvent, HotkeyManager};
use injector::{InjectionResult, TextInjector};
use polisher::Polisher;
use state::{AppState, RecordingState};
use transcriber::Transcriber;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Emitter, Listener, Manager,
};

// Commands sent from event listeners to the recording control thread.
// The control thread is the sole owner of AudioCapture (cpal::Stream is !Send).
enum RecordingCmd {
    Start,
    Stop,
    Cancel,
}

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
            commands::sign_in_with_key,
            commands::sign_out,
            commands::open_sign_in,
            commands::get_audio_devices,
            commands::get_transcriber_ready,
            commands::mark_setup_complete,
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
                "signin" => { let _ = open::that("https://console.groq.com/keys"); }
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

    // ── Load Whisper model in background ──────────────────────────────────
    {
        let state = app.state::<AppState>();
        let tr = Arc::clone(&state.transcriber);
        let emit_handle = handle.clone();
        state.rt.spawn(async move {
            let h = emit_handle.clone();
            let on_progress = move |phase: &str, pct: u32| {
                let _ = h.emit("model-progress", serde_json::json!({ "phase": phase, "percent": pct }));
            };
            match Transcriber::new(on_progress).await {
                Ok(t) => {
                    *tr.lock().await = Some(t);
                    let _ = emit_handle.emit("model-progress", serde_json::json!({ "phase": "ready", "percent": 100 }));
                    let _ = emit_handle.emit("transcriber-ready", ());
                }
                Err(e) => {
                    log::error!("Whisper model load failed: {}", e);
                    let _ = emit_handle.emit("model-progress", serde_json::json!({ "phase": "error", "percent": 0, "message": e.to_string() }));
                }
            }
        });
    }

    // ── Shared channels (created here so hotkey thread can hold cmd_tx) ──────
    let (audio_tx, audio_rx) = bounded::<AudioEvent>(256);
    let (cmd_tx, cmd_rx) = bounded::<RecordingCmd>(16);
    let in_flight = Arc::new(AtomicI32::new(0));
    let transcription_done = Arc::new(tokio::sync::Notify::new());

    // ── Hotkey listener ────────────────────────────────────────────────────
    // The hotkey thread sends RecordingCmd directly — bypassing the hidden
    // Bubble webview entirely (hidden WebView2 windows don't process JS events).
    // It re-reads hotkey_choice from settings every ~1s so changes take effect
    // without restarting the app.
    {
        let state = app.state::<AppState>();
        let choice = state.settings.lock().unwrap().hotkey_choice;
        let settings_arc = Arc::clone(&state.settings);
        drop(state);

        let app_handle = handle.clone();
        let cmd_tx = cmd_tx.clone();
        std::thread::spawn(move || {
            let mut mgr = match HotkeyManager::new(choice) {
                Ok(m) => m,
                Err(e) => {
                    log::error!("Failed to register hotkey: {}", e);
                    return;
                }
            };
            let mut tick: u32 = 0;

            loop {
                std::thread::sleep(std::time::Duration::from_millis(5));

                // Re-read hotkey choice every ~1s (200 × 5ms) so Settings changes
                // take effect immediately without an app restart.
                tick = tick.wrapping_add(1);
                if tick % 200 == 0 {
                    if let Ok(s) = settings_arc.try_lock() {
                        mgr.reconfigure(s.hotkey_choice);
                    }
                }

                match mgr.poll() {
                    Some(HotkeyEvent::HoldStart) => {
                        let _ = cmd_tx.try_send(RecordingCmd::Start);
                        let _ = app_handle.emit("hotkey-down", ());
                    }
                    Some(HotkeyEvent::HoldEnd) => {
                        let _ = cmd_tx.try_send(RecordingCmd::Stop);
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

        // ✓ button and ✕ button still route through frontend events → cmd_tx
        app.listen("stop-recording", {
            let tx = cmd_tx.clone();
            move |_| { let _ = tx.try_send(RecordingCmd::Stop); }
        });
        app.listen("cancel-recording", {
            let tx = cmd_tx.clone();
            move |_| { let _ = tx.try_send(RecordingCmd::Cancel); }
        });

        // ── Recording control thread ───────────────────────────────────────
        // cpal::Stream is !Send so AudioCapture must live on a single thread.
        {
            let audio_tx = audio_tx.clone();
            let recording_state = Arc::clone(&recording_state);
            let pending_segments = Arc::clone(&pending_segments);
            let app_handle = app_handle.clone();

            std::thread::spawn(move || {
                let mut capture: Option<AudioCapture> = None;

                for cmd in cmd_rx {
                    match cmd {
                        RecordingCmd::Start => {
                            if capture.is_some() { continue; }
                            {
                                let mut rs = recording_state.lock().unwrap();
                                if *rs != RecordingState::Idle { continue; }
                                *rs = RecordingState::Recording;
                            }
                            pending_segments.lock().unwrap().clear();

                            let mut cap = match AudioCapture::new(audio_tx.clone()) {
                                Ok(c) => c,
                                Err(e) => {
                                    log::error!("Audio capture error: {}", e);
                                    *recording_state.lock().unwrap() = RecordingState::Idle;
                                    continue;
                                }
                            };
                            if let Err(e) = cap.start() {
                                log::error!("Audio start error: {}", e);
                                *recording_state.lock().unwrap() = RecordingState::Idle;
                                continue;
                            }
                            capture = Some(cap);
                            if let Some(w) = app_handle.get_webview_window("bubble") {
                                let _ = w.show();
                            }
                            let _ = app_handle.emit("recording-started", ());
                        }
                        RecordingCmd::Stop => {
                            if let Some(mut cap) = capture.take() {
                                // Blocks ~300ms for tail drain, then sends
                                // AudioEvent::Chunk (flush) + RecordingStopped.
                                cap.stop();
                            }
                        }
                        RecordingCmd::Cancel => {
                            // Drop without stop() so RecordingStopped is never
                            // sent and the bridge thread skips processing.
                            drop(capture.take());
                            pending_segments.lock().unwrap().clear();
                            *recording_state.lock().unwrap() = RecordingState::Idle;
                            if let Some(w) = app_handle.get_webview_window("bubble") {
                                let _ = w.hide();
                            }
                            let _ = app_handle.emit("recording-cancelled", ());
                        }
                    }
                }
            });
        }

        // ── Audio bridge thread ────────────────────────────────────────────
        let bridge_handle = app_handle.clone();
        let bridge_transcriber = Arc::clone(&transcriber);
        let bridge_auth = Arc::clone(&auth);
        let bridge_history = Arc::clone(&history);
        let bridge_settings = Arc::clone(&settings);
        let bridge_segments = Arc::clone(&pending_segments);
        let bridge_recording = Arc::clone(&recording_state);
        let bridge_rt = rt.clone();
        let bridge_in_flight = Arc::clone(&in_flight);
        let bridge_done = Arc::clone(&transcription_done);

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

                        let auth = Arc::clone(&bridge_auth);
                        let history = Arc::clone(&bridge_history);
                        let settings_snap = bridge_settings.lock().unwrap().clone();
                        let emit = bridge_handle.clone();
                        let rec_state = Arc::clone(&bridge_recording);
                        let segs_ref = Arc::clone(&bridge_segments);
                        let in_flight = Arc::clone(&bridge_in_flight);
                        let done_notify = Arc::clone(&bridge_done);

                        bridge_rt.spawn(async move {
                            // Wait for any in-flight chunk transcriptions to land
                            while in_flight.load(Ordering::Relaxed) > 0 {
                                done_notify.notified().await;
                            }

                            let final_segments = segs_ref.lock().unwrap().clone();
                            if final_segments.is_empty() {
                                *rec_state.lock().unwrap() = RecordingState::Idle;
                                let _ = emit.emit("recording-cancelled", ());
                                return;
                            }

                            let app_ctx = context::capture();
                            let recent: Vec<_> = {
                                let h = history.lock().unwrap();
                                h.recent_for_app(&app_ctx.process_name, 3)
                                    .into_iter()
                                    .cloned()
                                    .collect()
                            };

                            let polisher = Polisher::new(auth.token());
                            let text = polisher
                                .polish(
                                    &final_segments,
                                    settings_snap.cleanup_level,
                                    app_ctx.category,
                                    Some(&settings_snap.user_profile),
                                    &recent,
                                )
                                .await;

                            let mut injector = TextInjector::new();
                            let result = injector.inject(&text);
                            let injected = matches!(result, InjectionResult::Success(_));
                            let message = if injected {
                                "Inserted ✓".to_string()
                            } else {
                                "Copied — press Ctrl+V".to_string()
                            };

                            let raw = final_segments
                                .iter()
                                .map(|s| s.text.as_str())
                                .collect::<Vec<_>>()
                                .join(" ");
                            history.lock().unwrap().push(history::HistoryEntry::new(
                                text.clone(),
                                Some(raw),
                                app_ctx.process_name,
                            ));
                            segs_ref.lock().unwrap().clear();

                            *rec_state.lock().unwrap() = RecordingState::Idle;
                            let _ = emit.emit(
                                "dictation-complete",
                                serde_json::json!({ "text": text, "message": message, "injected": injected }),
                            );
                            // Hide bubble after showing the done message for 2s
                            tokio::time::sleep(std::time::Duration::from_millis(2000)).await;
                            if let Some(w) = emit.get_webview_window("bubble") {
                                let _ = w.hide();
                            }
                        });
                    }
                    Ok(AudioEvent::Chunk(chunk)) => {
                        let tr = Arc::clone(&bridge_transcriber);
                        let segs = Arc::clone(&bridge_segments);
                        let in_flight = Arc::clone(&bridge_in_flight);
                        let done_notify = Arc::clone(&bridge_done);
                        let pause = chunk.pause_before_secs;
                        in_flight.fetch_add(1, Ordering::Relaxed);
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
                            drop(guard);
                            if in_flight.fetch_sub(1, Ordering::Relaxed) == 1 {
                                done_notify.notify_waiters();
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

    // First run → onboarding; subsequent launches → nothing (access via tray)
    let setup_complete = app.state::<AppState>().settings.lock().unwrap().setup_complete;
    if setup_complete {
        // nothing — user knows what to do
    } else {
        open_onboarding_window(app.handle());
    }

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

fn open_onboarding_window(app: &tauri::AppHandle) {
    if let Some(w) = app.get_webview_window("onboard") {
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let _ = tauri::WebviewWindowBuilder::new(
        app,
        "onboard",
        tauri::WebviewUrl::App("index.html#onboard".into()),
    )
    .title("Welcome to Hush")
    .inner_size(480.0, 360.0)
    .resizable(false)
    .center()
    .build();
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
