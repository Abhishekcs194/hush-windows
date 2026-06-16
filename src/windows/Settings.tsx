import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Keyboard, User, Info, Zap } from "lucide-react";
import {
  getSettings,
  saveSettings,
  getAuthState,
  getAudioDevices,
  getTranscriberReady,
  openSignIn,
  signInWithKey,
  signOut,
  Settings,
  AuthInfo,
} from "../lib/tauri";
import { onAuthChanged, onModelProgress, ModelProgressPayload } from "../lib/events";

type Tab = "general" | "hotkey" | "account" | "about";

const TABS: { id: Tab; label: string; icon: React.ReactNode }[] = [
  { id: "general", label: "General", icon: <Zap size={15} /> },
  { id: "hotkey", label: "Hotkey", icon: <Keyboard size={15} /> },
  { id: "account", label: "Account", icon: <User size={15} /> },
  { id: "about", label: "About", icon: <Info size={15} /> },
];

const CLEANUP_LEVELS = ["Off", "Light", "Standard", "Polished"] as const;
const HOTKEY_CHOICES = ["RightAlt", "RightCtrl", "RightShift", "CapsLock", "F13", "F14"] as const;
const HOTKEY_LABELS: Record<string, string> = {
  RightAlt: "Right Alt",
  RightCtrl: "Right Ctrl",
  RightShift: "Right Shift",
  CapsLock: "Caps Lock",
  F13: "F13",
  F14: "F14",
};

export default function SettingsWindow() {
  const [tab, setTab] = useState<Tab>("general");
  const [settings, setSettings] = useState<Settings | null>(null);
  const [auth, setAuth] = useState<AuthInfo | null>(null);
  const [devices, setDevices] = useState<string[]>([]);
  const [saved, setSaved] = useState(false);
  const [modelProgress, setModelProgress] = useState<ModelProgressPayload | null>(null);

  useEffect(() => {
    Promise.all([getSettings(), getAuthState(), getAudioDevices(), getTranscriberReady()]).then(
      ([s, a, d, ready]) => {
        setSettings(s);
        setAuth(a);
        setDevices(d);
        if (!ready) setModelProgress({ phase: "loading", percent: 0 });
      }
    );

    const unlistenAuth = onAuthChanged((signedIn) => {
      setAuth((prev) => prev ? { ...prev, signed_in: signedIn } : prev);
    });
    const unlistenModel = onModelProgress((p) => {
      setModelProgress(p.phase === "ready" ? null : p);
    });
    return () => {
      unlistenAuth.then((u) => u());
      unlistenModel.then((u) => u());
    };
  }, []);

  const update = (patch: Partial<Settings>) => {
    setSettings((prev) => prev ? { ...prev, ...patch } : prev);
  };

  const handleSave = async () => {
    if (!settings) return;
    await saveSettings(settings);
    setSaved(true);
    setTimeout(() => setSaved(false), 1500);
  };

  if (!settings) {
    return (
      <div className="flex items-center justify-center h-screen bg-surface text-zinc-500 text-sm">
        Loading…
      </div>
    );
  }

  return (
    <div className="flex h-screen bg-surface text-zinc-100 overflow-hidden">
      {/* Sidebar */}
      <aside className="w-44 flex-shrink-0 bg-zinc-950 flex flex-col py-4 gap-1 border-r border-border">
        <div className="px-4 pb-3 text-xs font-semibold text-zinc-500 uppercase tracking-wider">
          Settings
        </div>
        {TABS.map((t) => (
          <button
            key={t.id}
            onClick={() => setTab(t.id)}
            className={`flex items-center gap-2.5 mx-2 px-3 py-2 rounded-lg text-sm transition-colors text-left
              ${tab === t.id
                ? "bg-zinc-800 text-accent font-medium"
                : "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-900"
              }`}
          >
            <span className={tab === t.id ? "text-accent" : "text-zinc-500"}>
              {t.icon}
            </span>
            {t.label}
          </button>
        ))}
      </aside>

      {/* Content */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {/* Model loading banner */}
        {modelProgress && (
          <div className="border-b border-border bg-zinc-900 px-6 py-3 flex-shrink-0">
            <div className="flex items-center gap-3">
              <div className="flex-1 min-w-0">
                <div className="flex justify-between items-baseline mb-1.5">
                  <span className="text-xs font-medium text-zinc-200">
                    {modelProgress.phase === "downloading"
                      ? "Downloading Whisper model…"
                      : modelProgress.phase === "loading"
                      ? "Loading Whisper model into memory…"
                      : modelProgress.phase === "error"
                      ? "Model failed to load"
                      : "Initializing…"}
                  </span>
                  {modelProgress.phase === "downloading" && (
                    <span className="text-xs text-zinc-500 ml-2 tabular-nums">
                      {modelProgress.percent}%
                    </span>
                  )}
                </div>
                <div className="h-1 bg-zinc-800 rounded-full overflow-hidden">
                  {modelProgress.phase === "error" ? (
                    <div className="h-full bg-red-500 w-full" />
                  ) : modelProgress.phase === "downloading" ? (
                    <div
                      className="h-full bg-accent transition-all duration-200"
                      style={{ width: `${modelProgress.percent}%` }}
                    />
                  ) : (
                    <div className="h-full bg-accent/50 animate-pulse w-full" />
                  )}
                </div>
                {modelProgress.phase === "error" && modelProgress.message && (
                  <p className="text-xs text-red-400 mt-1">{modelProgress.message}</p>
                )}
              </div>
            </div>
          </div>
        )}

        <div className="flex-1 overflow-y-auto p-6">
          <motion.div
            key={tab}
            initial={{ opacity: 0, y: 4 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.12 }}
          >
            {tab === "general" && (
              <GeneralTab settings={settings} devices={devices} update={update} />
            )}
            {tab === "hotkey" && (
              <HotkeyTab settings={settings} update={update} />
            )}
            {tab === "account" && auth && (
              <AccountTab
                auth={auth}
                onSignOut={() => { signOut(); setAuth({ ...auth, signed_in: false }); }}
                onKeySet={() => setAuth({ ...auth, signed_in: true })}
              />
            )}
            {tab === "about" && <AboutTab />}
          </motion.div>
        </div>

        {/* Footer save bar */}
        <div className="border-t border-border px-6 py-3 flex justify-end items-center gap-3">
          {saved && (
            <span className="text-xs text-green-400">Saved ✓</span>
          )}
          <button
            onClick={handleSave}
            className="px-4 py-1.5 rounded-lg bg-accent hover:bg-pink-500 text-white text-sm font-medium transition-colors"
          >
            Save
          </button>
        </div>
      </main>
    </div>
  );
}

// ── Tabs ────────────────────────────────────────────────────────────────────

function GeneralTab({ settings, devices, update }: {
  settings: Settings;
  devices: string[];
  update: (p: Partial<Settings>) => void;
}) {
  return (
    <div className="space-y-6">
      <h2 className="text-base font-semibold">General</h2>

      <Field label="AI Cleanup Level" hint="How much the AI rewrites your speech.">
        <Select
          value={settings.cleanup_level}
          options={CLEANUP_LEVELS.map((v) => ({ value: v, label: v }))}
          onChange={(v) => update({ cleanup_level: v as Settings["cleanup_level"] })}
        />
      </Field>

      <Field label="About You" hint="Name, role, domain — sent to the AI for personalised corrections.">
        <textarea
          value={settings.user_profile}
          onChange={(e) => update({ user_profile: e.target.value })}
          rows={3}
          className="w-full bg-surface-overlay border border-border rounded-lg px-3 py-2
                     text-sm text-zinc-100 placeholder-zinc-600 focus:outline-none
                     focus:ring-1 focus:ring-accent resize-none"
          placeholder="e.g. Software engineer, working mostly in TypeScript and Rust."
        />
      </Field>

      <Field label="Microphone">
        <Select
          value={settings.input_device ?? "default"}
          options={[
            { value: "default", label: "Default" },
            ...devices.map((d) => ({ value: d, label: d })),
          ]}
          onChange={(v) => update({ input_device: v === "default" ? null : v })}
        />
      </Field>
    </div>
  );
}

function HotkeyTab({ settings, update }: {
  settings: Settings;
  update: (p: Partial<Settings>) => void;
}) {
  return (
    <div className="space-y-6">
      <h2 className="text-base font-semibold">Hotkey</h2>

      <Field label="Dictation Key" hint="The key you hold to start dictating.">
        <Select
          value={settings.hotkey_choice}
          options={HOTKEY_CHOICES.map((v) => ({ value: v, label: HOTKEY_LABELS[v] }))}
          onChange={(v) => update({ hotkey_choice: v as Settings["hotkey_choice"] })}
        />
      </Field>

      <Field label="Trigger Mode">
        <div className="space-y-2">
          <Toggle
            label="Hold to talk"
            hint="Hold the key while speaking, release to transcribe."
            checked={settings.hold_to_talk}
            onChange={(v) => update({ hold_to_talk: v })}
          />
          <Toggle
            label="Double-tap for hands-free"
            hint="Double-tap to latch on. Double-tap again to stop."
            checked={settings.hands_free}
            onChange={(v) => update({ hands_free: v })}
          />
        </div>
      </Field>
    </div>
  );
}

function AccountTab({ auth, onSignOut, onKeySet }: {
  auth: AuthInfo;
  onSignOut: () => void;
  onKeySet: () => void;
}) {
  const [key, setKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState("");

  const handleSaveKey = async () => {
    if (!key.trim()) return;
    setSaving(true);
    setError("");
    try {
      await signInWithKey(key.trim());
      setKey("");
      onKeySet();
    } catch (e: any) {
      setError(e?.toString() ?? "Failed to save key");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="space-y-6">
      <h2 className="text-base font-semibold">Account</h2>

      <div className="rounded-xl border border-border bg-surface-raised p-5 space-y-4">
        {auth.signed_in ? (
          <>
            <div className="flex items-center gap-3">
              <div className="w-8 h-8 rounded-full bg-accent flex items-center justify-center text-white text-sm font-semibold">
                G
              </div>
              <div>
                <div className="text-sm font-medium">Groq API key saved</div>
                <div className="text-xs text-zinc-500">Transcription &amp; AI cleanup enabled</div>
              </div>
            </div>
            <button
              onClick={onSignOut}
              className="w-full py-2 rounded-lg border border-border text-sm text-zinc-300
                         hover:bg-surface-overlay transition-colors"
            >
              Remove Key
            </button>
          </>
        ) : (
          <>
            <p className="text-sm text-zinc-400">
              Dictation works fully offline — no key needed. Add a free Groq key
              to enable optional AI text cleanup (Stage 2 polish).{" "}
              <button
                onClick={() => openSignIn()}
                className="text-accent hover:underline"
              >
                Get a free key →
              </button>
            </p>
            <div className="space-y-2">
              <input
                type="password"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleSaveKey()}
                placeholder="gsk_…"
                className="w-full bg-surface-overlay border border-border rounded-lg px-3 py-2
                           text-sm text-zinc-100 placeholder-zinc-600 focus:outline-none
                           focus:ring-1 focus:ring-accent font-mono"
              />
              {error && <p className="text-xs text-red-400">{error}</p>}
              <button
                onClick={handleSaveKey}
                disabled={saving || !key.trim()}
                className="w-full py-2 rounded-lg bg-accent hover:bg-pink-500 text-white text-sm
                           font-medium transition-colors disabled:opacity-50"
              >
                {saving ? "Saving…" : "Save API Key"}
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  );
}

function AboutTab() {
  return (
    <div className="space-y-4">
      <h2 className="text-base font-semibold">About Hush</h2>
      <div className="rounded-xl border border-border bg-surface-raised p-5 space-y-3">
        <div className="text-2xl font-bold text-accent">Hush</div>
        <div className="text-xs text-zinc-500">Version 0.1.0</div>
        <p className="text-sm text-zinc-400">
          Fast, private, system-wide voice dictation for Windows.
        </p>
        <div className="pt-2 space-y-1">
          <div className="text-xs text-zinc-600">Stage 1 — Whisper base.en (on-device, offline)</div>
          <div className="text-xs text-zinc-600">Stage 2 — Groq llama-3.1-8b-instant (optional)</div>
        </div>
      </div>
    </div>
  );
}

// ── Shared UI primitives ─────────────────────────────────────────────────────

function Field({ label, hint, children }: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="space-y-1.5">
      <label className="text-sm font-medium text-zinc-200">{label}</label>
      {hint && <p className="text-xs text-zinc-500">{hint}</p>}
      {children}
    </div>
  );
}

function Select({ value, options, onChange }: {
  value: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
}) {
  return (
    <select
      value={value}
      onChange={(e) => onChange(e.target.value)}
      className="w-full bg-surface-overlay border border-border rounded-lg px-3 py-2
                 text-sm text-zinc-100 focus:outline-none focus:ring-1 focus:ring-accent
                 appearance-none cursor-pointer"
    >
      {options.map((o) => (
        <option key={o.value} value={o.value}>
          {o.label}
        </option>
      ))}
    </select>
  );
}

function Toggle({ label, hint, checked, onChange }: {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (v: boolean) => void;
}) {
  return (
    <div
      className="flex items-center justify-between py-2.5 px-3 rounded-lg bg-surface-raised
                 border border-border cursor-pointer hover:border-zinc-600 transition-colors"
      onClick={() => onChange(!checked)}
    >
      <div>
        <div className="text-sm text-zinc-200">{label}</div>
        {hint && <div className="text-xs text-zinc-500 mt-0.5">{hint}</div>}
      </div>
      {/* Toggle pill */}
      <div
        className={`relative w-9 h-5 rounded-full transition-colors flex-shrink-0
          ${checked ? "bg-accent" : "bg-zinc-700"}`}
      >
        <div
          className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-transform
            ${checked ? "translate-x-4" : "translate-x-0.5"}`}
        />
      </div>
    </div>
  );
}
