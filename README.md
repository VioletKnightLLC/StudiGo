# StudiGo

**A creator-grade Windows studio: turn your GoPro into a live webcam, composite
it with your screen and local media, and record — all in one desktop app.**

StudiGo is a [Tauri 2](https://tauri.app) desktop application (Rust core +
React/TypeScript UI) that turns a GoPro Hero 8 into a system webcam, adds
multi-source compositing (GoPro + screen + imported media), and records the
composited output.

> **License:** Proprietary — see [`LICENSE`](LICENSE). Private, non-commercial,
> personal use only. Commercial use requires a license from Violet Knight LLC.

---

## Features

- **GoPro as webcam** — capture from a GoPro over USB. Hero 9+ appears as a
  native UVC camera; Hero 8 automatically falls back to screen capture of the
  GoPro Webcam Utility window.
- **Multi-source compositing** — combine **GoPro + Screen + imported media**
  into one live preview with single, Picture-in-Picture, side-by-side, and 2×2
  grid layouts.
- **Media library** — import local **images and videos** via a native file
  picker, toggle them into the composite, and place them next to your camera
  feed.
- **AI Stitch & Edit** — select your clips and a built-in AI agent (local Ollama
  vision + reasoning models) watches each one, chooses the most compelling
  moments, orders them, and stitches them into a single publish-ready MP4 with
  crossfade transitions. **Original files are never modified.**
- **Record the composite** — capture the live preview to a file with a
  3-second countdown, REC indicator, and duration timer.
- **Screen capture** — use any window or the full desktop as a source.
- **Virtual camera output** — expose the composited feed to Zoom / Teams / Meet
  / OBS (DirectShow filter).

---

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Shell | Tauri 2 (WebView2) |
| Core | Rust (tokio, rusb, nokhwa/MSMF) |
| UI | React 19 + TypeScript + Vite |
| Output | DirectShow virtual camera filter |

---

## Getting Started

### Prerequisites

- Windows 10/11
- Rust toolchain (stable)
- Node.js ≥ 20 + npm
- GoPro Webcam Utility (for GoPro sources)
- **FFmpeg + ffprobe** on PATH (used by the AI stitcher) — or set `STUDIGO_FFMPEG` to the ffmpeg.exe path/dir
- **Ollama** running locally with a vision model (e.g. `gemma4-e4b-it-vision`)
  for the AI Stitch feature's clip analysis

### Run in development

```bash
# Terminal 1 — Rust core + Tauri shell (also builds the UI)
cd src-tauri
cargo tauri dev
```

The UI is served by Vite on port 5173 during development.

### Build for distribution

```bash
# Frontend
cd ui
npm ci
npm run build

# Tauri app (Rust + UI + package)
cd src-tauri
npm run tauri build
```

### Project structure

```
src-tauri/          Rust core: source capture, compositor, virtual camera
  src/lib.rs          Tauri commands + plugin wiring
  src/sources/        UVC (GoPro), screen, fake sources
  src/compositor.rs   Scene compositing
  src/output/         DirectShow virtual camera
ui/                 React + TypeScript frontend
  src/App.tsx         Main app: sources, media library, composite, recording
docs/               Scoping + architecture docs
```

---

## Usage

1. **Connect a source**: check the box next to a source.
   - *Screen* — pick a window/desktop when prompted.
   - *GoPro* — auto-detects the camera. Hero 9+ uses native USB; Hero 8 falls
     back to recording the GoPro Webcam Utility window.
   - *Generic Camera* — any other USB webcam.
3. **Import media**: click **Import Media Files**, pick images/videos, and they
   appear in the Media Library. Check the box to include each in the composite.
4. **AI Stitch**: import ≥ 2 video clips, click **Stitch Clips with AI**, and the
   agent produces a polished combined MP4 (saved to `Documents/StudiGo/`).
   Originals are untouched.
5. **Choose a layout**: Single, PiP, Side-by-Side, or 2×2 Grid.
6. **Record**: press **Record** (or `Shift+R`) — a 3-second countdown runs,
   then the composited output is saved as a WebM file. Press Stop (`Shift+R`).

> **Hero 8 note:** the Hero 8 has no native UVC webcam mode. Connect it, enable
> **GoPro Connect** on the camera (Prefs → Connections → USB Connection), start
> the **GoPro Webcam Utility**, and use the **GoPro** source — StudiGo will
> capture its preview window for you.

---

## Keyboard Shortcuts

| Action | Key |
|--------|-----|
| Start / stop recording | `Shift+R` or `Ctrl+R` |
| Maximize preview | `F11` (restore: `Esc`) |
| Toggle settings | `S` |

---

## Roadmap

- [x] Multi-source compositing (GoPro + Screen)
- [x] Media library import (images + videos) + compositing
- [x] Composite recording with countdown
- [ ] Native UVC capture pipeline for Hero 9+ (MSMF)
- [ ] DirectShow virtual camera registration (Zoom/Teams/OBS)
- [ ] NLE timeline for editing recorded/imported clips

---

## License

Proprietary. See [`LICENSE`](LICENSE). Copyright © 2026 Violet Knight LLC.