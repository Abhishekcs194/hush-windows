import { useEffect, useState } from "react";
import { emit } from "@tauri-apps/api/event";
import { motion, AnimatePresence } from "framer-motion";
import Waveform from "../components/Waveform";
import {
  onRecordingStarted,
  onRecordingCancelled,
  onProcessingStarted,
  onLevelUpdate,
  onDictationComplete,
} from "../lib/events";

type Phase = "idle" | "recording" | "processing" | "done";

export default function Bubble() {
  const [phase, setPhase] = useState<Phase>("idle");
  const [message, setMessage] = useState("");

  useEffect(() => {
    const unlisten: Array<() => void> = [];

    // Backend shows the window and controls visibility.
    // Bubble only updates phase for rendering.
    onRecordingStarted(() => setPhase("recording")).then((u) => unlisten.push(u));

    onRecordingCancelled(() => setPhase("idle")).then((u) => unlisten.push(u));

    onProcessingStarted(() => setPhase("processing")).then((u) => unlisten.push(u));

    onLevelUpdate((rms: number) => {
      (Waveform as any)._push?.(rms);
    }).then((u) => unlisten.push(u));

    onDictationComplete((payload) => {
      setMessage(payload.message);
      setPhase("done");
      // Backend hides the window after 2s; reset phase to idle for next show
      setTimeout(() => setPhase("idle"), 2100);
    }).then((u) => unlisten.push(u));

    return () => unlisten.forEach((u) => u());
  }, []);

  const handleCancel = () => {
    emit("cancel-recording");
  };

  const handleFinish = () => {
    emit("stop-recording");
  };

  return (
    <div className="w-full h-full flex items-center justify-center">
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
              <button
                onClick={handleCancel}
                className="absolute left-4 w-7 h-7 rounded-full flex items-center justify-center
                           text-red-400 hover:bg-zinc-700 transition-colors text-sm no-drag"
              >
                ✕
              </button>
              <div className="w-48 h-full flex items-center">
                <Waveform active={true} />
              </div>
              <button
                onClick={handleFinish}
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
              <span className="text-zinc-400 text-sm">Transcribing…</span>
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
