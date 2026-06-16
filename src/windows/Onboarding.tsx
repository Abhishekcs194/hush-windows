import { Fragment, useEffect, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { motion, AnimatePresence } from "framer-motion";
import { onModelProgress, onTranscriberReady, ModelProgressPayload } from "../lib/events";

const HOTKEY_LABELS: Record<string, string> = {
  CtrlSpace:  "Ctrl + Space",
  WinSpace:   "Win + Space",
  RightAlt:   "Right Alt",
  RightCtrl:  "Right Ctrl",
  RightShift: "Right Shift",
  CapsLock:   "Caps Lock",
  F13: "F13",
  F14: "F14",
};

interface Settings { hotkey_choice: string }

export default function Onboarding() {
  const [progress, setProgress] = useState<ModelProgressPayload>({ phase: "loading", percent: 0 });
  const [hotkey, setHotkey] = useState("Ctrl + Space");
  const win = getCurrentWindow();

  const ready = progress.phase === "ready";

  useEffect(() => {
    invoke<Settings>("get_settings").then((s) => {
      setHotkey(HOTKEY_LABELS[s.hotkey_choice] ?? s.hotkey_choice);
    });
    invoke<boolean>("get_transcriber_ready").then((r) => {
      if (r) setProgress({ phase: "ready", percent: 100 });
    });

    const unlisten: Array<() => void> = [];
    onModelProgress((p) => setProgress(p)).then((u) => unlisten.push(u));
    onTranscriberReady(() => setProgress({ phase: "ready", percent: 100 })).then((u) => unlisten.push(u));
    return () => unlisten.forEach((u) => u());
  }, []);

  const handleStart = async () => {
    await invoke("mark_setup_complete");
    win.close();
  };

  const keys = hotkey.split("+").map((k) => k.trim());

  return (
    <div className="w-full h-full flex flex-col" style={{ background: "#111113" }}>

      {/* Top brand strip */}
      <div className="flex flex-col items-center pt-10 pb-8 px-10">
        <div className="w-14 h-14 rounded-[18px] flex items-center justify-center mb-4 shadow-lg"
             style={{ background: "linear-gradient(145deg, #f74a9e 0%, #a855f7 100%)" }}>
          <MicIcon />
        </div>
        <h1 className="text-white text-2xl font-semibold tracking-tight mb-1">Hush</h1>
        <p className="text-zinc-500 text-sm">AI voice dictation · works in every app</p>
      </div>

      {/* Divider */}
      <div className="mx-10 h-px" style={{ background: "rgba(255,255,255,0.06)" }} />

      {/* Main content */}
      <div className="flex-1 flex flex-col items-center justify-center px-10 py-8">
        <AnimatePresence mode="wait">
          {!ready ? (
            <motion.div key="loading"
              initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }}
              className="w-full flex flex-col items-center gap-3">
              <p className="text-zinc-300 text-sm font-medium">
                {progress.phase === "downloading"
                  ? "Downloading AI model…"
                  : progress.phase === "error"
                    ? "Download failed"
                    : "Loading AI model…"}
              </p>
              <div className="w-full h-1 rounded-full overflow-hidden" style={{ background: "rgba(255,255,255,0.08)" }}>
                <motion.div className="h-full rounded-full"
                  style={{ background: "linear-gradient(90deg, #f74a9e, #a855f7)" }}
                  animate={{
                    width: progress.phase === "downloading"
                      ? `${progress.percent}%`
                      : progress.phase === "loading"
                        ? "65%"
                        : "0%"
                  }}
                  transition={{ duration: 0.5 }}
                />
              </div>
              <p className="text-zinc-600 text-xs">
                {progress.phase === "downloading"
                  ? `${progress.percent}% · ~150 MB · one-time download`
                  : "Almost ready…"}
              </p>
            </motion.div>
          ) : (
            <motion.div key="ready"
              initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }}
              className="w-full flex flex-col items-center gap-6">

              {/* Hotkey display */}
              <div className="flex flex-col items-center gap-3">
                <p className="text-zinc-400 text-sm">Hold to dictate · release to transcribe</p>
                <div className="flex items-center gap-2">
                  {keys.map((k, i) => (
                    <Fragment key={k}>
                      <kbd
                        className="px-3.5 py-2 rounded-lg text-white text-sm font-medium"
                        style={{
                          background: "rgba(255,255,255,0.07)",
                          border: "1px solid rgba(255,255,255,0.14)",
                          boxShadow: "0 2px 0 rgba(0,0,0,0.4)",
                          fontFamily: "inherit",
                        }}>
                        {k}
                      </kbd>
                      {i < keys.length - 1 && (
                        <span className="text-zinc-600 text-sm font-light">+</span>
                      )}
                    </Fragment>
                  ))}
                </div>
              </div>

              {/* Description */}
              <p className="text-zinc-500 text-xs text-center leading-relaxed">
                Text appears in any app — browser, Word, Slack — right where your cursor is.<br />
                Change the shortcut anytime in <span className="text-zinc-400">Settings → Hotkey</span>.
              </p>
            </motion.div>
          )}
        </AnimatePresence>
      </div>

      {/* Bottom CTA */}
      <div className="px-10 pb-8 flex flex-col items-center gap-3">
        <button
          onClick={handleStart}
          disabled={!ready}
          className="w-full py-2.5 rounded-xl text-white text-sm font-semibold
                     transition-all disabled:opacity-40 hover:brightness-110 active:scale-[0.98]"
          style={{ background: ready ? "linear-gradient(135deg, #f74a9e 0%, #a855f7 100%)" : "#333" }}
        >
          {ready ? "Start using Hush →" : "Setting up…"}
        </button>
        {ready && (
          <p className="text-zinc-600 text-xs">
            For smarter cleanup, add a Groq API key in{" "}
            <span className="text-zinc-400">Settings → Account</span>
          </p>
        )}
      </div>
    </div>
  );
}

function MicIcon() {
  return (
    <svg width="24" height="24" viewBox="0 0 24 24" fill="none"
         stroke="white" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <rect x="9" y="2" width="6" height="12" rx="3" />
      <path d="M5 10a7 7 0 0 0 14 0" />
      <line x1="12" y1="19" x2="12" y2="22" />
      <line x1="9" y1="22" x2="15" y2="22" />
    </svg>
  );
}
