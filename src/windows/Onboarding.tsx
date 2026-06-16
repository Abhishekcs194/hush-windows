import { useEffect, useState } from "react";
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
  F13:        "F13",
  F14:        "F14",
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

  return (
    <div className="w-full h-full flex flex-col items-center justify-center gap-8 px-10 select-none"
         style={{ background: "rgba(18,18,20,1)" }}>

      {/* Logo */}
      <div className="flex flex-col items-center gap-1">
        <div className="w-12 h-12 rounded-2xl flex items-center justify-center"
             style={{ background: "linear-gradient(135deg, #f74a9e 0%, #a855f7 100%)" }}>
          <MicIcon />
        </div>
        <span className="text-white text-xl font-semibold mt-2">Hush</span>
        <span className="text-zinc-500 text-sm">Voice dictation for Windows</span>
      </div>

      {/* Status area */}
      <AnimatePresence mode="wait">
        {!ready ? (
          <motion.div key="loading"
            initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -8 }}
            className="w-full flex flex-col items-center gap-3">
            <p className="text-zinc-400 text-sm">
              {progress.phase === "downloading"
                ? `Downloading AI model… ${progress.percent}%`
                : progress.phase === "error"
                  ? `Error: ${(progress as any).message ?? "Download failed"}`
                  : "Loading AI model…"}
            </p>
            <div className="w-full h-1.5 rounded-full bg-zinc-800 overflow-hidden">
              <motion.div
                className="h-full rounded-full"
                style={{ background: "linear-gradient(90deg, #f74a9e, #a855f7)" }}
                animate={{ width: progress.phase === "loading" ? "60%" : `${progress.percent}%` }}
                transition={{ duration: 0.4 }}
              />
            </div>
            <p className="text-zinc-600 text-xs">This only happens once</p>
          </motion.div>
        ) : (
          <motion.div key="ready"
            initial={{ opacity: 0, y: 8 }} animate={{ opacity: 1, y: 0 }}
            className="w-full flex flex-col items-center gap-6">

            {/* Hotkey demo */}
            <div className="flex flex-col items-center gap-2">
              <p className="text-zinc-400 text-sm">Hold to dictate, release to transcribe</p>
              <div className="flex items-center gap-1.5">
                {hotkey.split("+").map((k) => (
                  <span key={k} className="px-3 py-1.5 rounded-lg text-white text-sm font-mono font-medium"
                        style={{ background: "rgba(255,255,255,0.08)", border: "1px solid rgba(255,255,255,0.12)" }}>
                    {k.trim()}
                  </span>
                ))}
              </div>
              <p className="text-zinc-600 text-xs mt-1">
                Change anytime in Settings → Hotkey
              </p>
            </div>

            <button
              onClick={handleStart}
              className="px-6 py-2.5 rounded-full text-white text-sm font-medium transition-opacity hover:opacity-90 active:opacity-75"
              style={{ background: "linear-gradient(135deg, #f74a9e 0%, #a855f7 100%)" }}
            >
              Start using Hush →
            </button>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}

function MicIcon() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="white" strokeWidth="2"
         strokeLinecap="round" strokeLinejoin="round">
      <rect x="9" y="2" width="6" height="12" rx="3" />
      <path d="M5 10a7 7 0 0 0 14 0" />
      <line x1="12" y1="19" x2="12" y2="22" />
      <line x1="9" y1="22" x2="15" y2="22" />
    </svg>
  );
}
