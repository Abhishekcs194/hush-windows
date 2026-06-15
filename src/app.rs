use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::{Arc, Mutex};
use tokio::runtime::Handle;

use crate::audio::{AudioCapture, AudioEvent};
use crate::auth::AuthStore;
use crate::context;
use crate::history::{History, HistoryEntry};
use crate::hotkey::{HotkeyChoice, HotkeyEvent, HotkeyManager};
use crate::injector::{InjectionResult, TextInjector};
use crate::polisher::{CleanupLevel, Polisher};
use crate::transcriber::{TranscriptionSegment, Transcriber};
use crate::ui::{
    bubble::{BubblePhase, BubbleState},
    settings::SettingsState,
    tray::{Tray, TrayAction},
};

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq)]
enum RecordingState {
    Idle,
    HoldToTalk,
    HandsFree,
}

struct TranscriptionChunk {
    segment: TranscriptionSegment,
}

pub struct App {
    // Core services
    auth: Arc<AuthStore>,
    history: Arc<Mutex<History>>,
    polisher: Arc<tokio::sync::Mutex<Polisher>>,

    // Audio pipeline
    audio: Option<AudioCapture>,
    audio_rx: Receiver<AudioEvent>,
    audio_tx: Sender<AudioEvent>,

    // Transcription state
    segments: Vec<TranscriptionSegment>,
    transcriber: Arc<tokio::sync::Mutex<Option<Transcriber>>>,

    // Channel for results coming back from async transcription tasks
    chunk_tx: Sender<TranscriptionChunk>,
    chunk_rx: Receiver<TranscriptionChunk>,

    // Pending polish result (Some = waiting for async task)
    pending_polish: Option<Arc<Mutex<Option<String>>>>,
    // Countdown to hide the bubble after showing Done state
    bubble_hide_at: Option<std::time::Instant>,

    // Input handling
    hotkey: HotkeyManager,
    injector: TextInjector,
    recording_state: RecordingState,

    // Tokio handle
    rt: Handle,

    // UI state (public so main.rs can render them)
    pub bubble: BubbleState,
    pub settings: SettingsState,
    pub tray: Option<Tray>,
    pub show_history: bool,
    pub should_quit: bool,
}

impl App {
    pub fn new(rt: Handle) -> anyhow::Result<Self> {
        let (audio_tx, audio_rx) = bounded::<AudioEvent>(256);
        let (chunk_tx, chunk_rx) = bounded::<TranscriptionChunk>(32);

        let auth = Arc::new(AuthStore::new());
        let history = Arc::new(Mutex::new(History::load()));
        let polisher = Arc::new(tokio::sync::Mutex::new(Polisher::new(auth.token())));

        let transcriber: Arc<tokio::sync::Mutex<Option<Transcriber>>> =
            Arc::new(tokio::sync::Mutex::new(None));
        {
            let tr = Arc::clone(&transcriber);
            rt.spawn(async move {
                match Transcriber::new().await {
                    Ok(t) => {
                        *tr.lock().await = Some(t);
                        log::info!("Transcriber ready");
                    }
                    Err(e) => log::error!("Failed to load Whisper model: {}", e),
                }
            });
        }

        let hotkey = HotkeyManager::new(HotkeyChoice::default())?;

        let is_signed_in = auth.is_signed_in();
        let user_email = String::new(); // populated after auth
        let tray = Tray::new(is_signed_in).ok();

        let mut settings = SettingsState::new(APP_VERSION.to_string());
        settings.signed_in = is_signed_in;
        settings.user_email = user_email;
        settings.available_devices =
            crate::audio::AudioCapture::list_devices().unwrap_or_default();

        Ok(Self {
            auth,
            history,
            polisher,
            audio: None,
            audio_rx,
            audio_tx,
            segments: Vec::new(),
            transcriber,
            chunk_tx,
            chunk_rx,
            pending_polish: None,
            bubble_hide_at: None,
            hotkey,
            injector: TextInjector::new(),
            recording_state: RecordingState::Idle,
            rt,
            bubble: BubbleState::new(),
            settings,
            tray,
            show_history: false,
            should_quit: false,
        })
    }

    /// Drive all state — call once per egui frame.
    pub fn update(&mut self, ctx: &egui::Context) {
        self.poll_hotkey();
        self.poll_audio_events();
        self.poll_transcription_chunks();
        self.poll_pending_polish();
        self.poll_tray();
        self.check_bubble_hide();

        if self.recording_state != RecordingState::Idle {
            ctx.request_repaint();
        }
    }

    // ── Hotkey ─────────────────────────────────────────────────────────────

    fn poll_hotkey(&mut self) {
        match self.hotkey.poll() {
            Some(HotkeyEvent::HoldStart) => {
                if self.recording_state == RecordingState::Idle {
                    self.start_recording(false);
                }
            }
            Some(HotkeyEvent::HoldEnd) => {
                if self.recording_state == RecordingState::HoldToTalk {
                    self.stop_recording();
                }
            }
            Some(HotkeyEvent::DoubleTap) => match self.recording_state {
                RecordingState::Idle => self.start_recording(true),
                RecordingState::HandsFree => self.stop_recording(),
                _ => {}
            },
            None => {}
        }
    }

    // ── Audio ───────────────────────────────────────────────────────────────

    fn poll_audio_events(&mut self) {
        while let Ok(event) = self.audio_rx.try_recv() {
            match event {
                AudioEvent::LevelUpdate(rms) => {
                    self.bubble.push_level(rms);
                }
                AudioEvent::Chunk(chunk) => {
                    let tr = Arc::clone(&self.transcriber);
                    let tx = self.chunk_tx.clone();
                    let pause = chunk.pause_before_secs;
                    self.rt.spawn(async move {
                        let guard = tr.lock().await;
                        if let Some(t) = guard.as_ref() {
                            match t.transcribe(&chunk.samples, chunk.sample_rate, pause).await {
                                Ok(seg) => {
                                    let _ = tx.send(TranscriptionChunk { segment: seg });
                                }
                                Err(e) => log::warn!("Transcription error: {}", e),
                            }
                        }
                    });
                }
                AudioEvent::RecordingStarted => {
                    self.bubble.phase = BubblePhase::Recording;
                    self.bubble.reset_levels();
                }
                AudioEvent::RecordingStopped => {
                    // Give async tasks a moment to deliver last chunk, then polish
                    self.bubble.phase = BubblePhase::Processing;
                    // We schedule polish after a short drain window (handled next frame via flag)
                    self.begin_polish();
                }
            }
        }
    }

    fn poll_transcription_chunks(&mut self) {
        while let Ok(chunk) = self.chunk_rx.try_recv() {
            if !chunk.segment.text.is_empty() {
                self.segments.push(chunk.segment);
            }
        }
    }

    fn start_recording(&mut self, hands_free: bool) {
        self.segments.clear();
        self.pending_polish = None;

        let audio_tx = self.audio_tx.clone();
        match AudioCapture::new(audio_tx) {
            Ok(mut capture) => {
                if let Err(e) = capture.start() {
                    log::error!("Failed to start audio: {}", e);
                    return;
                }
                self.audio = Some(capture);
                self.recording_state = if hands_free {
                    RecordingState::HandsFree
                } else {
                    RecordingState::HoldToTalk
                };
            }
            Err(e) => log::error!("Failed to create audio capture: {}", e),
        }
    }

    fn stop_recording(&mut self) {
        if let Some(mut audio) = self.audio.take() {
            audio.stop(); // blocking tail drain inside
        }
        self.recording_state = RecordingState::Idle;
    }

    // ── Polish ──────────────────────────────────────────────────────────────

    fn begin_polish(&mut self) {
        if self.segments.is_empty() {
            self.bubble.phase = BubblePhase::Hidden;
            return;
        }

        let result: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let result_w = Arc::clone(&result);
        self.pending_polish = Some(result);

        let segments = self.segments.clone();
        let polisher = Arc::clone(&self.polisher);
        let auth_token = self.auth.token();
        let level = self.settings.cleanup_level;
        let app_ctx = context::capture();
        let profile = self.settings.user_profile.clone();
        let history = Arc::clone(&self.history);

        self.rt.spawn(async move {
            let recent: Vec<_> = {
                let h = history.lock().unwrap();
                h.recent_for_app(&app_ctx.process_name, 3)
                    .into_iter()
                    .cloned()
                    .collect()
            };

            let mut p = polisher.lock().await;
            p.set_token(auth_token);
            let text = p
                .polish(&segments, level, app_ctx.category, Some(&profile), &recent)
                .await;
            *result_w.lock().unwrap() = Some(text);
        });
    }

    fn poll_pending_polish(&mut self) {
        let result = match &self.pending_polish {
            Some(r) => Arc::clone(r),
            None => return,
        };

        let text_opt = result.lock().unwrap().clone();
        if let Some(text) = text_opt {
            self.pending_polish = None;
            self.inject_and_record(text);
        }
    }

    fn inject_and_record(&mut self, text: String) {
        let result = self.injector.inject(&text);
        let app_ctx = context::capture();

        let message = match result {
            InjectionResult::Success(_) => "Inserted ✓".to_string(),
            InjectionResult::Copied => "Copied — press Ctrl+V".to_string(),
        };

        self.bubble.phase = BubblePhase::Done { message };
        self.bubble_hide_at =
            Some(std::time::Instant::now() + std::time::Duration::from_secs(2));

        let raw_text: String = self
            .segments
            .iter()
            .map(|s| s.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let entry = HistoryEntry::new(text, Some(raw_text), app_ctx.process_name);
        self.history.lock().unwrap().push(entry);
        self.segments.clear();
    }

    fn check_bubble_hide(&mut self) {
        if let Some(hide_at) = self.bubble_hide_at {
            if std::time::Instant::now() >= hide_at {
                self.bubble_hide_at = None;
                self.bubble.phase = BubblePhase::Idle;
            }
        }
    }

    // ── Tray ────────────────────────────────────────────────────────────────

    fn poll_tray(&mut self) {
        let action = self.tray.as_ref().and_then(|t| t.poll());
        match action {
            Some(TrayAction::ShowSettings) => self.settings.open = true,
            Some(TrayAction::ShowHistory) => self.show_history = true,
            Some(TrayAction::SignIn) => self.open_sign_in_browser(),
            Some(TrayAction::SignOut) => {
                self.auth.sign_out();
                self.settings.signed_in = false;
            }
            Some(TrayAction::Quit) => self.should_quit = true,
            None => {}
        }
    }

    // ── Public UI callbacks ─────────────────────────────────────────────────

    pub fn on_bubble_cancel(&mut self) {
        self.segments.clear();
        self.pending_polish = None;
        self.bubble.phase = BubblePhase::Hidden;
        self.injector.restore_clipboard();
        if self.recording_state != RecordingState::Idle {
            self.stop_recording();
        }
    }

    pub fn on_bubble_finish(&mut self) {
        if self.recording_state != RecordingState::Idle {
            self.stop_recording();
        }
    }

    pub fn on_settings_event(&mut self, event: crate::ui::settings::SettingsEvent) {
        use crate::ui::settings::SettingsEvent;
        match event {
            SettingsEvent::CleanupLevelChanged(level) => self.settings.cleanup_level = level,
            SettingsEvent::HotkeyChanged(choice) => {
                if let Err(e) = self.hotkey.reconfigure(choice) {
                    log::error!("Hotkey reconfigure error: {}", e);
                }
            }
            SettingsEvent::TriggerModeChanged { hold, free } => {
                self.settings.hold_to_talk = hold;
                self.settings.hands_free = free;
            }
            SettingsEvent::UserProfileChanged(p) => self.settings.user_profile = p,
            SettingsEvent::InputDeviceChanged(_d) => {
                // Recreate audio capture with the new device next time recording starts
            }
            SettingsEvent::SignInRequested => self.open_sign_in_browser(),
            SettingsEvent::SignOutRequested => {
                self.auth.sign_out();
                self.settings.signed_in = false;
            }
            SettingsEvent::Closed => self.settings.open = false,
        }
    }

    pub fn on_auth_token(&mut self, token: String) {
        self.auth.sign_in(token);
        self.settings.signed_in = true;
    }

    fn open_sign_in_browser(&self) {
        let url = crate::auth::pair_url();
        if let Err(e) = open::that(&url) {
            log::error!("Failed to open browser: {}", e);
        }
    }
}
