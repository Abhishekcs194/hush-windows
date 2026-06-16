import { listen, Event } from "@tauri-apps/api/event";

export interface DictationCompletePayload {
  text: string;
  message: string;
  injected: boolean;
}

export const onRecordingStarted = (cb: () => void) =>
  listen("recording-started", cb);

export const onRecordingCancelled = (cb: () => void) =>
  listen("recording-cancelled", cb);

export const onProcessingStarted = (cb: () => void) =>
  listen("processing-started", cb);

export const onLevelUpdate = (cb: (rms: number) => void) =>
  listen<number>("level-update", (e: Event<number>) => cb(e.payload));

export const onDictationComplete = (cb: (payload: DictationCompletePayload) => void) =>
  listen<DictationCompletePayload>("dictation-complete", (e) => cb(e.payload));

export const onTranscriberReady = (cb: () => void) =>
  listen("transcriber-ready", cb);

export interface ModelProgressPayload {
  phase: "downloading" | "loading" | "ready" | "error";
  percent: number;
  message?: string;
}

export const onModelProgress = (cb: (p: ModelProgressPayload) => void) =>
  listen<ModelProgressPayload>("model-progress", (e) => cb(e.payload));

export const onAuthChanged = (cb: (signedIn: boolean) => void) =>
  listen<boolean>("auth-changed", (e) => cb(e.payload));
