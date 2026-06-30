# GoPro Webcam Studio — Project Context for Coding Agents

> **You are a coding agent operating inside the `gopro-studio` Hermes profile.**
> Read this file in full before writing any code. It is the source of truth
> for architecture, contracts, directory layout, and how your work is judged.

## Project goal

A Windows desktop application that turns a GoPro Hero 8 Black (USB UVC) into a
system webcam, with light non-linear video editing and screen-share (via
WebView2 `getDisplayMedia`) built in.

**Tech stack:** Tauri 2.0 (Rust core + TypeScript/React UI via WebView2),
FFmpeg (`ffmpeg-next` crate) for encode/decode, a bundled DirectShow source
filter for the virtual camera output.

## Architectural overview

```
┌─────────────────────────────────────────────────────────────────┐
│                       GoPro Webcam Studio                       │
│                                                                 │
│  GoPro UVC  │  Screen (getDisplayMedia)  │  File import          │
│       └──────────────┬───────────────────┘                       │
│                      ▼                                          │
│           ┌────────────────────┐   ┌──────────────────────┐     │
│           │  Source Bus +      │──▶│  Compositor         │     │
│           │  Compositor (Rust) │   │  (scene graph)      │     │
│           └────────┬───────────┘   └─────────┬────────────┘     │
│                    │                         │                │
│         ┌──────────┴─────────┐                ▼                │
│         ▼                    ▼     ┌──────────────────────┐    │
│   NLE timeline         Virtual cam   │ Preview canvas      │    │
│   (Rust + web UI)     (DirectShow    │ (WebView2 + WebGL)  │    │
│                        filter)       └──────────────────────┘    │
│         │                    │                                    │
│         ▼                    ▼                                    │
│   FFmpeg export         Zoom / Teams / OBS / Meet                  │
│   (H.264 MP4)                                                  │
└─────────────────────────────────────────────────────────────────┘
```

## Directory layout (target)

```
gopro-webcam-studio/
├── AGENTS.md                      # this file — read first
├── .hermes.md                     # Hermes-specific behavior overrides
├── docs/
│   ├── scope.md                   # full scoping document (read for "why")
│   ├── contracts/                 # frozen interfaces (one file per contract)
│   │   ├── source_bus.md
│   │   ├── scene.md
│   │   ├── compositor.md
│   │   ├── virtual_cam_pipe.md
│   │   ├── project.md
│   │   └── exporter.md
│   └── project.schema.json        # JSON Schema for the NLE Project type
├── spikes/                        # throwaway Phase 0 de-risking code
│   ├── T0-A/
│   ├── T0-B/
│   ├── T0-C/
│   └── SYNTHESIS.md
├── src-tauri/                     # Rust core
│   ├── Cargo.toml
│   ├── src/
│   │   ├── lib.rs
│   │   ├── source_bus.rs          # SourceBus trait, Frame type, FakeSource
│   │   ├── screen_capture.rs      # ScreenCaptureSource: SourceBus
│   │   ├── gopro_source.rs        # GoProSource: SourceBus (UVC over USB)
│   │   ├── compositor.rs          # Compositor trait + Scene render
│   │   ├── transitions.rs
│   │   ├── text_overlay.rs
│   │   ├── virtual_cam.rs         # named-pipe writer + frame protocol
│   │   ├── recording.rs           # local MP4 capture
│   │   ├── importer.rs            # file -> frame stream
│   │   ├── timeline.rs            # Project timeline model + ops
│   │   ├── exporter.rs
│   │   └── firmware_check.rs
│   ├── tests/                     # integration tests, one per contract
│   └── examples/                  # preview_gopro, etc.
├── ui/                            # React + TypeScript (Tauri webview)
│   ├── package.json
│   ├── src/
│   │   ├── App.tsx
│   │   ├── scenes/                # scene picker UI
│   │   ├── preview/               # WebGL preview canvas
│   │   ├── timeline/              # NLE timeline UI
│   │   └── ipc.ts                 # Tauri IPC marshalling
│   └── vite.config.ts
├── filter/                        # DirectShow virtual cam (C++)
│   ├── CMakeLists.txt
│   ├── src/
│   └── installer/                 # regsvr32 / WiX fragment
├── installer/                     # Tauri NSIS bundler config
└── .github/workflows/ci.yml
```

## Contracts —Frozen interfaces (DO NOT modify without an orchestrator task)

Cross-task boundaries are written interfaces. Each contract has a doc under
`docs/contracts/<name>.md` and a Rust trait or JSON schema backing it. If your
task needs to change a contract, **stop and report** — do not edit it inline.
Contract changes are orchestrator-owned.

| Contract | Defined by | Lives in |
|---|---|---|
| `SourceBus` | trait `async fn next_frame(&mut self) -> Result<Frame>`; `Frame = ndarray::Array3<u8>` RGB at declared fps | `src-tauri/src/source_bus.rs` + `docs/contracts/source_bus.md` |
| `Scene` | JSON: `{ sources: [{id, transform, z_index, alpha}], audio: {...}, resolution: [w,h], fps: N }` | `docs/contracts/scene.md` |
| `Compositor` | `fn render(&self, scene, timestamps) -> Frame` | `src-tauri/src/compositor.rs` |
| `VirtualCamPipe` | Named pipe `\\.\pipe\gopro-studio-cam`; writer pushes `[u32 width][u32 height][u32 stride][u64 pts_ns][bytes BGRA]` | `src-tauri/src/virtual_cam.rs` + `docs/contracts/virtual_cam_pipe.md` |
| `Project` | JSON describing timeline, clips, transitions, text, export settings | `docs/contracts/project.md` + `docs/project.schema.json` |
| `Exporter` | `async fn export(project, out_path, progress_cb) -> Result<()>` | `src-tauri/src/exporter.rs` |

Until a contract exists, do not reference it. When you implement a contract,
update its doc in the same commit.

## Per-task agent brief template

Every task you receive will be phrased like this — read it literally:

```
TASK: <title>
REPO: C:/Users/lemik/Documents/gopro-webcam-studio
WORKDIR: <repo path>   (defaults to repo root; may be a feature subdirectory)
BACKEND: claude-code | codex

GOAL (one sentence):
<what this task accomplishes>

CONTRACTS YOU MUST OBEY (read before writing code):
- <verbatim Rust trait / JSON schema, or pointer to docs/contracts/X.md>
- <upstream artifact path, e.g. "spikes/T0-B/src/uvc.rs">

INPUTS (artifacts this task reads):
- <file paths produced by parent tasks>

OUTPUTS (artifacts this task produces):
- <file paths and a one-line description of each>

ACCEPTANCE PREDICATE (a verifier runs this; you must make it pass):
- <exact shell command(s) using cargo test / ffprobe / file existence>
  Expected: <deterministic output or exit code>

DO NOT:
- touch files outside <repo path>
- modify any contract in docs/contracts/ (those are orchestrator-owned)
- introduce dependencies not in Cargo.toml / package.json without a TASK-TYPE:dep-add sibling task

VERIFY YOURSELF BEFORE EXITING:
- Run the acceptance predicate. Paste the exact output into your final
  message. If it fails, you have not completed the task.

COMMIT:
- One commit, message: "<type>: <task-id> <one-line>".
- Push only if instructed.
```

## Rules for every task

1. **Run the acceptance predicate yourself before declaring done.** Paste the
   exact command and its verbatim output in your final message. "It should
   work" is not done.
2. **One commit per task.** Subject: `<type>: <task-id> <one-line>`. Types:
   `feat`, `fix`, `refactor`, `docs`, `chore`, `test`, `spike`.
3. **Do not edit contracts.** They live in `docs/contracts/` and are owned by
   the orchestrator. If a contract blocks you, finish what you can and report
   the conflict — do not patch the contract.
4. **Do not introduce dependencies** not already in `Cargo.toml` / `package.json`
   without explicit permission in the task brief.
5. **TDD.** Every code task ships a test that fails before the impl exists and
   passes after. See `software-development/test-driven-development` skill.
6. **Do not run `git push` unless the brief says to.** Local commits only.
7. **Stay inside the repo path** given in the brief. Do not touch
   `~/.hermes/`, other projects, or anything outside the repo.
8. **Speak the project's contract language.** A frame is an
   `ndarray::Array3<u8>` RGB. A scene is the `Scene` JSON. Don't invent
   parallel types.
9. **Spikes are throwaway.** Code under `spikes/<id>/` is not production code
   and is never imported by `src-tauri/` directly. When a task "adopts" a
   spike, it copies the relevant code into the production tree with fresh
   types and tests.
10. **Use the `context7` MCP server** to look up Tauri / `ffmpeg-next` /
    `nusb` / DirectShow API docs instead of guessing. The `filesystem` MCP is
    scoped to this repo for path-pinned file ops.

## Coding standards

- **Rust:** edition 2021, `#![deny(warnings)]` not required but fix all
  clippy warnings. Public functions carry rustdoc comments. No `unsafe`
  without a `// SAFETY:` justification comment.
- **TypeScript:** strict mode. Function components + hooks. No `any` without
  a `// why:` comment.
- **C++ (DirectShow filter):** C++17. COM-smart pointers (`Microsoft::WRL::ComPtr`).
  No raw `new`/`delete` outside allocator shims.
- **Tests:** `cargo test` for Rust, `vitest` for TS, plus a run script
  (`scripts/verify_<task-id>.sh`) for any acceptance predicate that needs
  more than `cargo test <name>`.
- **Naming:** snake_case for Rust, camelCase for TS, PascalCase for C++ types.

## Common build / test commands

```bash
# Rust core
cd src-tauri && cargo build --release
cd src-tauri && cargo test --workspace
cd src-tauri && cargo test source_bus::tests -- --nocapture
cd src-tauri && cargo doc --no-deps --open

# UI
cd ui && npm install && npm run dev
cd ui && npm run build
cd ui && npm run test

# Tauri shell (builds Rust + UI + bundles)
cargo tauri dev
cargo tauri build

# DirectShow filter (requires MSVC + Windows SDK)
cd filter && cmake -B build -S . && cmake --build build --config Release
cd filter && regsvr32 build/Release/gopro_studio_cam.dll

# Acceptance predicates (per task)
bash scripts/verify_<task-id>.sh
```

## When you get stuck

1. **Read the relevant contract doc** (`docs/contracts/<name>.md`) and the
   spike that proved the integration (under `spikes/`).
2. **Use `context7`** for library API questions — don't guess from memory.
3. **Use `mcp_filesystem_search_files`** to find usages of the type you're
   implementing.
4. **If you genuinely cannot proceed** (a contract is wrong, an upstream
   artifact is missing, an API you assumed doesn't exist), **exit with a
   BLOCKED message** describing: (a) what you tried, (b) the exact error,
   (c) what you need. Do not silently downgrade the task to something easier.

## Reference documents

- `docs/scope.md` — the full agentic-build scoping document (the "why" and
  the task graph). Read it once at the start of any new task for context.
- `docs/contracts/*.md` — frozen interfaces.
- `spikes/SYNTHESIS.md` — the Phase 0 verdicts; explains why each
  integration approach was chosen.
