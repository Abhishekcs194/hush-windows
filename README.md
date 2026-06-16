# Hush for Windows

AI voice dictation that works in every app. Hold a key, speak, release — your words appear at the cursor.

## Download

**[→ Download the latest release](../../releases/latest)**

Grab the `.msi` installer (recommended) or the portable `.exe`.

> **Requirements:** Windows 10 or 11 (64-bit)

---

## How it works

1. Launch Hush — it lives in the system tray
2. Click anywhere you want to type
3. Hold **Ctrl + Space**, speak naturally, release
4. Your transcribed text is injected at the cursor

The AI model (~150 MB) downloads automatically on first launch. No account required for basic use.

### Optional: smarter cleanup

Add a [Groq API key](https://console.groq.com/keys) in **Settings → Account** to enable LLM-powered text polish (punctuation, capitalisation, filler-word removal). The free tier is generous.

---

## Hotkeys

| Shortcut | Action |
|----------|--------|
| Hold **Ctrl + Space** | Start dictating |
| Release | Transcribe and insert |
| **✕** button on bubble | Cancel |
| **✓** button on bubble | Finish early |

Change the shortcut anytime in **Settings → Hotkey**.

---

## Build from source

### Prerequisites

- [Node.js 20+](https://nodejs.org/)
- [Rust (stable, MSVC toolchain)](https://rustup.rs/) — select `x86_64-pc-windows-msvc`
- [CMake](https://cmake.org/download/)
- Visual Studio 2022 with the **Desktop development with C++** workload

### Steps

```powershell
git clone https://github.com/Abhishekcs194/hush-windows.git
cd hush-windows
npm install

# Dev mode (hot-reload)
$env:CARGO_TARGET_DIR = "C:\hush-target"
npm run tauri dev

# Release build
npm run tauri build
# Output: src-tauri/target/release/bundle/
```

The `.cargo/config.toml` in the repo sets `WHISPER_DONT_GENERATE_BINDINGS=1` automatically — no LLVM required.

---

## Architecture

| Layer | Technology |
|-------|-----------|
| UI | Tauri v2 + React + TypeScript |
| Transcription | whisper.cpp (on-device, `ggml-base.en`) |
| Text polish | Groq LLM API (optional) |
| Text injection | Win32 clipboard + SendInput |
| Hotkey detection | Win32 `GetAsyncKeyState` polling |

---

## License

MIT
