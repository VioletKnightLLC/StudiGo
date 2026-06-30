# GoPro Webcam Studio — Scoping Document

**Goal:** A Windows desktop application that turns a GoPro Hero 8 Black into a system webcam, and adds lightweight video editing and screen-share capabilities around that video source.

**Document type:** Scoping / feasibility + architecture sketch (not yet a task-by-task build plan). The output of this doc is a shared understanding of what we're building, why, and how — before committing to a build plan.

---

## 1. What the user actually wants (problem statement)

A single desktop app that does three jobs:

1. **GoPro-as-webcam** — make the Hero 8 appear to Zoom / Teams / Meet / OBS as a normal webcam, USB-tethered.
2. **Video editing** — record or import clips (incl. from the GoPro), do quick NLE edits (trim, split, transitions, text, audio), and export.
3. **Screen share** — capture the desktop / a window / a region, and expose that same virtual camera feed (or a composite of camera + screen) to video conferencing apps.

Implicit goal: a "creator-grade" single workstation for calls, recording, and light editing, replacing the triad of (GoPro Webcam Utility + OBS + a separate NLE).

---

## 2. Feasibility per feature (what's known, what's risky)

### 2.1 GoPro Hero 8 as webcam

The Hero 8 Black has **two** viable paths to turn it into a webcam. Both have well-documented prior art; there is no green-field reverse-engineering risk.

| Path | How it works | Latency | Cabling | Reliability |
|------|--------------|---------|---------|-------------|
| **A. USB UVC mode (official GoPro Webcam Utility approach)** | Hero 8 firmware ≥ v2.0 exposes a UVC (USB Video Class) device over USB-C. OS sees it as a standard webcam. This is exactly what GoPro's own Webcam Utility relies on. | ~150–250 ms | USB-C to USB-A/C, ideally USB 3.x | High — it's a real webcam to the OS |
| **B. RTMP / RTSP over Wi-Fi** | Hero 8 hosts `udp://10.5.5.9:8554` (low-latency substream) or accepts an RTMP push. `ffmpeg` ingests it. KonradIT's GoProStream + many gists prove the recipe. | ~300–800 ms | Wi-Fi only (USB tether for power optional) | Medium — UDP drops, Wi-Fi congestion |

**Recommendation:** Path A (USB UVC) as primary. It's what the official utility uses, it's the path that "just looks like a webcam" to everything downstream (Zoom, Teams, OBS, our own app), and it avoids the latency/compression artifacts of Wi-Fi RTMP. Path B (RTMP) is a stretch goal for users who want to roam untethered.

From our app's perspective on Path A: the Hero 8 is **just a UVC camera**. We read it with the same video-capture stack we'd use for any webcam. The "GoPro-specific" knowledge is limited to:
- Helping the user update firmware (banner / link to GoPro Utility installer for the firmware side).
- Detecting the device by USB VID:PID (GoPro's UVC descriptor) so we can label it "GoPro Hero 8" in the UI rather than "USB Video Device".
- Quirk handling: Hero 8 sometimes needs a mode toggle to mount as UVC; we can detect and prompt.

### 2.2 Virtual camera output (the "make it a webcam to others" half)

To expose our processed/composited feed to Zoom/Teams/OBS, we need a **virtual camera device** on Windows. Options:

- **OBS Virtual Camera** — mature, ships a DirectShow virtual cam. We can either detect & piggyback on an OBS install, or bundle a standalone DirectShow source filter. Piggybacking is a 1-week shortcut but adds an OBS dependency; bundling is cleaner but ~2–4 weeks of C++ DirectShow work.
- **Unity Capture / e2eSoft VCam / VisioForge Virtual Camera SDK** — third-party DirectShow virtual cams, some with permissive licensing. VisioForge sells an SDK that does exactly this; ~$300/dev.
- **Roll our own** — a Windows DirectShow source filter that reads from a named pipe / shared memory we write frames into. This is the "real product" path — no dependency on OBS being installed, full control.

**Recommendation:** Phase 1 = detect & use OBS Virtual Camera if present (fastest to a working build). Phase 2 = bundle/ship our own DirectShow virtual cam as the productized path.

### 2.3 Video editing (light NLE)

This is the **highest-risk, highest-effort** feature. A real non-linear editor is a multi-year project. We are scoping a *light* NLE — enough to be useful, not a DaVinci replacement.

Realistic scope for v1:
- Import clips (from GoPro capture, screen recording, or file).
- Timeline: 1–2 video tracks, 1–2 audio tracks.
- Operations: trim, split, razor, delete, drag-to-move, snap.
- Transitions: crossfade, dip-to-black (parameterised).
- Text overlay (basic: position, font, size, color, duration).
- Audio: level adjust, fade in/out, simple ducking.
- Export: H.264 MP4, fixed presets (1080p30, 1080p60, 720p30), no custom encoder UI.

Tech options for the editing engine:

| Stack | Pros | Cons |
|-------|------|------|
| **Electron + FFmpeg + WebGL/Canvas for preview** | Web tech, fast to ship UI; `ffmpeg` does the actual encode; `getDisplayMedia` already in-browser for screen share | Preview rendering is JS-single-threaded; timeline perf with many clips can lag; no GPU-resident decode |
| **Tauri 2.0 + Rust + GStreamer (or ffmpeg via bindings)** | Smaller binary, better perf, Rust safety; GStreamer gives a graph-based pipeline close to an NLE's mental model | GStreamer on Windows is workable but not painless; smaller ecosystem than Electron |
| **Qt (C++ or PySide) + QtMultimedia + MLT framework** | MLT (used by Shotcut) is a real NLE engine; QtMultimedia handles capture/preview natively; C++ perf | Heaviest learning curve; MLT docs are thin; slower UI iteration |
| **Flutter + dart:ffi to a C/C++ engine (ffmpeg/MLT)** | Nice cross-platform UI; one codebase for later macOS/mobile | FFI plumbing for video frames is fiddly; fewer off-the-shelf video widgets |

**Recommendation:** **Tauri 2.0 (Rust core) + FFmpeg (via `ffmpeg-next` crate) for encode + a WebGL/Canvas preview layer**. Hits the sweet spot: small binary (relevant since we're bundling ffmpeg + maybe a virtual cam driver), real perf for the video pipeline, web-tech UI velocity, and Rust's safety matters once we're shuffling raw frames. Electron is the faster-UI fallback if Tauri's video/frame plumbing proves too thin.

### 2.4 Screen share

Straightforward on all three stacks thanks to the WebRTC `getDisplayMedia()` API (works in Chromium → works in Electron and Tauri's webview). On Windows, capture is DXGI-backed and efficient. We get a `MediaStream` we can:
- Route into the preview canvas.
- Composite with the GoPro feed (picture-in-picture, side-by-side, fullscreen + camera bubble).
- Pipe to the virtual camera output.

Quirk: in Tauri the webview is platform-native (WebView2 on Windows), and `getDisplayMedia` support there is solid as of 2024+. We'll verify with a spike in Phase 0.

---

## 3. Proposed architecture (target)

```
┌─────────────────────────────────────────────────────────────────┐
│                        GoPro Webcam Studio                      │
│                                                                 │
│  ┌─────────────┐   ┌─────────────┐   ┌──────────────────────┐  │
│  │ GoPro UVC   │   │ Screen cap  │   │ File import (clips)   │  │
│  │ (USB Hero8) │   │ (getDisplay │   │                      │  │
│  │             │   │  Media)     │   │                      │  │
│  └──────┬──────┘   └──────┬──────┘   └──────────┬───────────┘  │
│         │                 │                     │              │
│         └────────┬────────┴─────────────────────┘              │
│                  ▼                                              │
│        ┌────────────────────┐    ┌──────────────────────────┐   │
│        │  Source Bus /      │───▶│  Preview compositor      │   │
│        │  Compositor (Rust) │    │  (WebGL canvas in webview)│   │
│        └────────┬───────────┘    └──────────────────────────┘   │
│                 │                                               │
│        ┌────────┴───────────┐                                   │
│        ▼                    ▼                                    │
│  ┌──────────────┐    ┌──────────────────┐                        │
│  │ NLE timeline │    │ Virtual cam out  │                        │
│  │ (Rust model, │    │ (DirectShow src  │                        │
│  │  web UI)     │    │  filter → pipe)  │                        │
│  └──────┬───────┘    └────────┬─────────┘                        │
│         │                     │                                  │
│         ▼                     ▼                                  │
│  ┌──────────────┐    ┌──────────────────┐                        │
│  │ FFmpeg export│    │ Zoom / Teams /   │                        │
│  │ (H.264 MP4)  │    │ OBS / Meet see a │                        │
│  └──────────────┘    │ normal webcam    │                        │
│                      └──────────────────┘                        │
└─────────────────────────────────────────────────────────────────┘
```

**Key architectural decisions:**

1. **Source Bus** — an in-process pub/sub of decoded RGB/Grayscale frames from each source (GoPro UVC, screen capture, file decoder). Rust core owns this; web UI subscribes.
2. **Compositor** — combines sources per a scene description (camera-only, screen-only, PiP, side-by-side). Same compositor feeds both the on-screen preview and the virtual cam output, so "what you see is what Zoom gets".
3. **NLE is a separate, later-attached module** — the timeline is just another consumer of decoded frames + an exporter that calls FFmpeg. Keeping it decoupled means v1 can ship with just webcam+screen+virtual-cam, and editing lands as v2.
4. **Virtual cam as a DirectShow filter** — the only system-level installable component. Everything else is app-local. The filter reads from a named pipe our app writes to; the app is the single writer.

---

## 4. Phasing (deliverable milestones)

### Phase 0 — Spikes (1–2 weeks)
- Tauri 2.0 desktop skeleton on Windows; verify webview `getDisplayMedia` works.
- Detect Hero 8 over USB (VID:PID enumeration via `nusb` / `rusb`); capture a frame via UVC.
- Render that frame to a canvas at ≥30 fps.
- **Exit criterion:** a Tauri window showing a live GoPro 8 feed + a live screen capture, side by side.

### Phase 1 — "GoPro Webcam Studio (webcam + screen)" (4–6 weeks)
- Source Bus + Compositor (camera-only, screen-only, PiP, side-by-side scenes).
- Virtual camera output via OBS Virtual Camera detection (Phase 1a) or our own DirectShow filter (Phase 1b).
- Scene presets + hotkey switching.
- Local recording (write compositor output straight to MP4 via FFmpeg while streaming to virtual cam).
- Settings: resolution, frame rate, mirror, crop.
- **Exit criterion:** user can open Zoom, pick "GoPro Webcam Studio" as camera, and see a PiP of GoPro + screen.

### Phase 2 — Light NLE (6–8 weeks)
- Timeline UI (1–2 V tracks, 1–2 A tracks).
- Import from Phase 1 recordings or file.
- Trim / split / razor / move / snap.
- Crossfade + dip-to-black transitions.
- Text overlay.
- Audio levels, fades, ducking.
- Export presets (1080p30/60, 720p30) via FFmpeg.
- **Exit criterion:** user can record a call (GoPro + screen), open the recording in-app, trim it, add a title card, and export an MP4.

### Phase 3 — Polish & productize (3–4 weeks)
- Replace OBS-virtual-cam dependency with our own signed DirectShow filter.
- Auto-update, installer (Tauri's updater + NSIS/WiX).
- Firmware-update prompt for Hero 8.
- Wi-Fi RTMP path as an alternate source (stretch).
- Telemetry / crash reporting opt-in.
- **Exit criterion:** signed installer, no external dependencies, ships to a non-technical user.

**Total realistic timeline: ~14–20 weeks part-time / ~10–14 weeks full-time for v1 (Phases 0–2).**

---

## 5. Tech stack summary

| Layer | Choice | Notes |
|-------|--------|-------|
| Shell / UI | **Tauri 2.0 + TypeScript/React** | Small binary, WebView2 on Windows, web-tech UI velocity |
| Core / video pipeline | **Rust** | Source Bus, compositor, NLE model, FFmpeg bindings |
| Capture — GoPro | UVC over USB via `nusb` / platform media APIs | Hero 8 firmware ≥ v2.0; reads like any webcam |
| Capture — screen | `getDisplayMedia` in WebView2 | Verify in Phase 0 spike |
| Encode / decode / container | **FFmpeg** (`ffmpeg-next` crate + bundled binaries) | Standard, well-understood |
| Virtual camera (Win) | **DirectShow source filter** (C++), installed at setup | Phase 1: detect OBS VirtCam as fallback |
| NLE engine (Phase 2) | Custom Rust timeline model + FFmpeg export; consider `MLT` if transitions get complex | YAGNI until Phase 2 |
| Installer | Tauri's nsis/wix bundlers | Auto-update via Tauri updater |
| Crash/telemetry | Sentry (opt-in) | Standard |

**Alternative stack (fallback if Tauri video plumbing blocks us):** Electron + `ffmpeg-static` + node-FFI to a small Rust/C++ helper for frame handling. Larger binary, faster UI iteration.

---

## 6. Risks, tradeoffs, and open questions

### Risks
- **Hero 8 UVC reliability** — some units need a cable/USB-port combo to mount cleanly; some need a mode toggle. Mitigation: in-app " reconnect" button + a detected-device banner with a link to GoPro's firmware updater.
- **DirectShow filter signing** — Windows 10/11 increasingly insists on signed drivers. Our virtual cam filter will need an EV code-signing cert ($200–400/yr) and proper WHQL or attestation signing. This is a real cost & process step, not a code step.
- **WebView2 `getDisplayMedia` parity** — generally fine on Windows, but corner cases (multi-monitor, DPI scaling, capturing the webview's own window) need testing. Spike in Phase 0.
- **NLE scope creep** — the temptation to keep adding editor features is high and can blow the timeline. The Phase 2 exit criteria must be enforced.
- **Audio sync** — GoPro UVC carries audio over USB; screen capture carries system audio via WASAPI loopback. Keeping these in sync in the compositor and in exports is a known hard problem; budget debug time.

### Tradeoffs
- **Tauri vs Electron:** smaller binary & better perf vs more mature video-related npm ecosystem. We're betting Tauri's WebView2 catches up; Electron is the fallback.
- **Bundled virtual cam vs OBS dependency:** Phase 1 ships faster with OBS detection; Phase 3 productizes. Acceptable staged trade.
- **Roll-our-own NLE vs embed an existing engine (MLT/Shotcut's):** rolling our own gives a clean license & UX; embedding MLT is faster to a working editor but inherits its UI assumptions. Defer the decision to Phase 2 kickoff.

### Open questions for the user
1. **Windows-only, or do you want a macOS path too?** Hero 8 UVC works on both; virtual-cam path differs (DAL plugin on macOS vs DirectShow on Windows). Scoped above as Windows-first.
2. **Is the NLE a must-have for v1, or can Phase 1 ship webcam+screen+record and Phase 2 deliver editing?** My recommendation is the latter — it gets a useful product in your hands ~6 weeks sooner.
3. **Bundle OBS Virtual Camera detection at first, or only ship our own filter from day one?** Detecting OBS first saves ~3 weeks of DirectShow work and lets you start using the app immediately.
4. **Wi-Fi RTMP path (untethered GoPro) — needed, or a nice-to-have?** Adds latency & complexity; I'd defer to Phase 3.
5. **Do you need to record both GoPro + screen simultaneously, or just one at a time?** Affects compositor design.

---

## 7. What's next

Once you confirm the scope and answer the open questions in §6, the next step is to turn this document into a task-by-task build plan (using the `plan` skill's format) for **Phase 0 spikes** — the 1–2 week of technical de-risking that tells us whether Tauri is the right shell before committing to the full build.
