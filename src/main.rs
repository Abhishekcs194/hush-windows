#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod audio;
mod auth;
mod context;
mod history;
mod hotkey;
mod injector;
mod polisher;
mod transcriber;
mod ui;

use app::App;
use eframe::NativeOptions;
use std::sync::{Arc, Mutex};

fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    // Tokio runtime for async whisper + groq tasks
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let rt_handle = runtime.handle().clone();

    let app_state = Arc::new(Mutex::new(App::new(rt_handle)?));

    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_visible(false)         // hidden root window — tray-only
            .with_taskbar(false)
            .with_inner_size([1.0, 1.0]),
        ..Default::default()
    };

    // Keep tokio runtime alive alongside eframe
    let _rt = runtime;

    eframe::run_native(
        "Hush",
        options,
        Box::new(move |_cc| Ok(Box::new(HushEframe { state: app_state }))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {}", e))
}

struct HushEframe {
    state: Arc<Mutex<App>>,
}

impl eframe::App for HushEframe {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut app = self.state.lock().unwrap();

        // Drive state machine
        app.update(ctx);

        if app.should_quit {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Render secondary viewports
        let mut cancel = false;
        let mut finish = false;
        ui::bubble::show(ctx, &app.bubble, &mut cancel, &mut finish);

        if cancel {
            app.on_bubble_cancel();
        }
        if finish {
            app.on_bubble_finish();
        }

        let settings_events = ui::settings::show(ctx, &mut app.settings);
        for event in settings_events {
            app.on_settings_event(event);
        }

        // Root window is invisible; request repaint on a slow timer to keep
        // the tray and hotkey pollers alive without burning CPU
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        log::info!("Hush shutting down");
    }
}
