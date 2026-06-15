import { useEffect, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { motion, AnimatePresence } from "framer-motion";
import Waveform from "../components/Waveform";
import {
  onRecordingStarted,
  onProcessingStarted,
  onLevelUpdate,
  onDictationComplete,
} from "../lib/events";

type Phase = "idle" | "recording" | "processing" | "done";

export default function Bubble() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState("");
  const win = getCurrentWindow();

  useEffect(() => {
    const unlisten: Array<() => void> = [];

    onRecordingStarted(() => {
      setPhase("recording");
      win.show();
    }).then((u) => unlisten.push(u));

    onProcessingStarted(() => {
      setPhase("processing");
    }).then((u) => unlisten.push(u));

    onLevelUpdate((rms: number) => {
      (Waveform as any)._push?.(rms);
    }).then((u) => unlisten.push(u));

    onDictationComplete((payload) => {
      setMessage(payload.message);
      setPhase("done");
      setTimeout(() => {
        setPhase("idle");
        win.hide();
      }, 2000);
    }).then((u) => unlisten.push(u));

    return () => unlisten.forEach((u) => u());
  }, []);

  return (
    <div className="w-full h-full flex items-center justify-center">
      {/* Pill container */}
      <AnimatePresence mode="wait">
        <motion.div
          key={phase}
          initial={{ opacity: 0, scale: 0.95, y: 6 }}
          animate={{ opacity: 1, scale: 1, y: 0 }}
          exit={{ opacity: 0, scale: 0.95, y: 6 }}
          transition={{ duration: 0.15, ease: "easeOut" }}
          className="relative flex items-center justify-center w-[400px] h-[56px] rounded-full"
          style={{ background: "rgba(28,28,30,0.92)", backdropFilter: "blur(20px)" }}
        >
          {phase === "idle" && (
            <div className="w-32 h-1 rounded-full bg-zinc-700" />
          )}

          {phase === "recording" && (
            <>
              {/* Cancel button */}
              <button
                onClick={() => {
                  setPhase("idle");
                  win.hide();
                }}
                className="absolute left-4 w-7 h-7 rounded-full flex items-center justify-center
                           text-red-400 hover:bg-zinc-700 transition-colors text-sm no-drag"
              >
                ✕
              </button>

              {/* Waveform */}
              <div className="w-48 h-full flex items-center">
                <Waveform active={true} />
              </div>

              {/* Finish button */}
              <button
                onClick={() => setPhase("processing")}
                className="absolute right-4 w-7 h-7 rounded-full flex items-center justify-center
                           text-green-400 hover:bg-zinc-700 transition-colors text-sm no-drag"
              >
                ✓
              </button>
            </>
          )}

          {phase === "processing" && (
            <div className="flex items-center gap-2">
              <Spinner />
              <span className="text-zinc-400 text-sm">Polishing…</span>
            </div>
          )}

          {phase === "done" && (
            <span className="text-zinc-300 text-sm">{message}</span>
          )}
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

function Spinner() {
  return (
    <motion.div
      className="w-4 h-4 rounded-full border-2 border-zinc-600 border-t-accent"
      animate={{ rotate: 360 }}
      transition={{ repeat: Infinity, duration: 0.8, ease: "linear" }}
    />
  );
}
