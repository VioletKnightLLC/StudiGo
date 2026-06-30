# GoPro Webcam Studio — Agentic Build Scope

**Goal:** A Windows desktop app (GoPro Hero 8 → webcam + light NLE + screen share) built **entirely by an agent harness**, not a human developer.

**Document type:** Scope + task graph + per-task agent briefs, designed for **Agent Orchestration**. The "implementer" is a coding-agent harness (Claude Code / Codex / OpenCode / Hermes delegations). The human is the **operator** who approves the plan, handles a handful of gating decisions, and stages the fleet — not a coder.

---

## 1. What changes when the builder is an agent harness

A human-dev plan assumes the implementer reads the file, eyeballs the result, and course-corrects. An agent harness has none of that. The scope must absorb the harness's constraints:

| Human-dev assumption | Agentic-build reality | What the plan must do |
|---|---|---|
| Implementer knows what "looks right" | Implementer has no taste; judges only against explicit criteria | Every task carries **objective acceptance criteria** — predicate the verifier runs |
| Implementer reads related code by instinct | Implementer has no broader context | Each task prompt is **self-contained**: file paths, API contracts, links to upstream task outputs |
| Implementer persists knowledge in their head | Each agent's context is fresh per task | **Handoffs are artifacts**, not memories. A task can't say "use what we built earlier"; it says "import from `src/source_bus.rs` (see T3 contract)" |
| Debugging is interactive | Agent can't eyeball a running video window | Verifiable outputs are **machine-checkable** (exit codes, file existence, JSON shape, stdout markers), or a human verifies at a gated checkpoint |
| Slow loop = weeks | A 10-turn coding-agent loop runs in ~2-15 minutes | Tasks are sized to **one agent loop, ≤ ~30 min wall-clock** — not "2-5 min of focused human work" |
| Hard parts get heroics | Agent gives up or hallucinates on hard parts | High-risk work is **front-loaded as spikes** with go/no-go gates; the agent can't silently limp past a broken integration |

### Principles enforced in this scope

1. **One task = one agent loop.** A coding agent (Claude Code `-p` or Codex `exec`) receives a prompt, runs read→edit→test→commit, and exits. Multi-day epics are *decompositions*, not tasks.
2. **Contract-first.** Every cross-task boundary is a written interface (a Rust trait, a JSON schema, a file-path convention). Agents write to contracts, not vibes.
3. **Verifiable acceptance.** Each task specifies a command a verifier agent (or the operator) can run that returns a pass/fail. No "review the code" handoffs — that's a reviewer *task*, separate.
4. **Explicit dependency graph with fan-out/fan-in.** Independent tasks run in parallel; synthesis/validation tasks gate on their parents via kanban `parents=[...]` or explicit `--resume` chains.
5. **Spikes before commitment.** Risky integrations (Tauri+WebView2 video, DirectShow filter, UVC enumeration on Windows) ship as throwaway spike tasks with **verdict artifacts** before any production code references them.
6. **Human gates only where the cost of being wrong is irreversible or financial.** EV-code-signing cert purchase, the DirectShow-driver-path go/no-go, and ship/no-ship — that's it.

---

## 2. The harness (what's running what)

```
┌──────────────────────────────────────────────────────────────────────┐
│  OPERATOR (human — you)                                               │
│  • Approves this scope.                                               │
│  • Resolves gating decisions (signing cert, OS scope).               │
│  • Reads the kanban board & intervenes only on blocks.                │
└───────────────┬──────────────────────────────────────────────────────┘
                │ manages
                ▼
┌──────────────────────────────────────────────────────────────────────┐
│  ORCHESTRATOR  (Hermes main session — you, via this agent)            │
│  Role: decompose → spawn → review-handoff → merge.                    │
│  Does NOT write production code. Owns the kanban board.               │
│  Tools: delegate_task, kanban_*, cronjob, terminal, read_file.        │
│                                                                       │
│  Two execution backends (pick based on task):                         │
│  ┌────────────────────────┐   ┌────────────────────────┐              │
│  │ Claude Code (print -p) │   │ Codex CLI (exec)       │              │
│  │ — non-interactive      │   │ — pty + full-auto      │              │
│  │ --max-turns budget     │   │ — git-worktree parallel│              │
│  │ --output-format json   │   │ — best for greenfield  │              │
│  │ — best for edits/tests │   │   scaffolding          │              │
│  └────────────────────────┘   └────────────────────────┘              │
│                                                                       │
│  Optional durable layer:                                             │
│  • kanban board (SQLite) for cross-session task state                 │
│  • cronjob for nightly verification sweeps (build + test)            │
│  • session_search to recall past turn outputs when an agent needs    │
│    "what did T4 produce?" without re-running it                       │
└──────────────────────────────────────────────────────────────────────┘
```

**Why two coding backends:**
- **Claude Code print mode** is the cleaner default: JSON output, `--max-turns` budget, `--allowedTools` whitelist, session resumes for follow-ups. Used for edits, tests, refactors on existing files.
- **Codex `exec --full-auto`** shines on greenfield scaffolding (new crates, new packages) and parallel issue-fix runs using git worktrees — each worktree is an isolated filesystem for one agent.

Most tasks can run on either. The per-task briefs in §5 name a recommended backend; either works.

---

## 3. Architecture (target — the agents' destination)

```
┌─────────────────────────────────────────────────────────────────┐
│                      GoPro Webcam Studio                        │
│                                                                 │
│  ┌─────────────┐  ┌─────────────┐  ┌──────────────────────┐     │
│  │ GoPro UVC   │  │ Screen cap  │  │ File import (clips)  │     │
│  │ (USB Hero8) │  │ (getDisplay │  │                      │     │
│  │             │  │  Media)     │  │                      │     │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬───────────┘     │
│         └────────┬───────┴─────────────────────┘                │
│                  ▼                                              │
│        ┌────────────────────┐   ┌───────────────────────────┐   │
│        │  Source Bus +      │──▶│  Compositor (scene graph) │   │
│        │  Compositor (Rust)│   │                            │   │
│        └────────┬───────────┘   └─────────┬─────────────────┘   │
│                 │                         │                    │
│        ┌────────┴───────────┐             ▼                    │
│        ▼                    ▼   ┌───────────────────────────┐   │
│  ┌──────────────┐  ┌──────────────────┐ │ Preview canvas     │   │
│  │ NLE timeline │  │ Virtual cam out  │ │ (WebView2 + WebGL) │   │
│  │ (Rust model, │  │ (DirectShow src  │ └───────────────────┘   │
│  │  web UI)     │  │  filter → pipe)  │                          │
│  └──────┬───────┘  └────────┬─────────┘                          │
│         │                     │                                    │
│         ▼                     ▼                                    │
│  ┌──────────────┐    ┌──────────────────┐                         │
│  │ FFmpeg export│    │ Zoom / Teams /   │                         │
│  │ (H.264 MP4)  │    │ OBS / Meet …     │                         │
│  └──────────────┘    └──────────────────┘                         │
└─────────────────────────────────────────────────────────────────┘
```

**Contracts the agents write to** (locked first, before any feature code):

| Contract | Defined by task | What it guarantees |
|---|---|---|
| `SourceBus` trait | T3 | `async fn next_frame(&mut self) -> Result<Frame>`; frames are `ndarray::Array3<u8>` RGB at a declared fps |
| `Scene` JSON schema | T6 | `{ sources: [{id, transform, z_index, alpha}], audio: {...}, resolution: [w,h], fps: N }` |
| `Compositor` trait | T6 | `fn render(&self, scene, timestamps) -> Frame` |
| `VirtualCamPipe` protocol | T10 | Named pipe `\\.\pipe\gopro-studio-cam`; writer pushes raw BGRA + frame header; filter pulls |
| `Project` (NLE) schema | T15 | JSON describing timeline, clips, transitions, export settings |
| `Exporter` trait | T17 | `async fn export(project, out_path, progress_cb) -> Result<()>` |

---

## 4. Phasing — designed for the harness's loop cadence

Each phase is a **kanban release**: a set of tasks whose collective completion defines a verifiable milestone. Agents complete tasks; the orchestrator reports phase exit when every task's acceptance predicate passes.

### Phase 0 — Spikes (de-risk before commitment)
**Goal:** prove the 3 highest-risk integrations before any production code is written.
**Cadence:** 3 parallel agents → 1 synthesis gate.
**Exit gate:** synthesis task T0-SYN returns VALIDATED/PARTIAL/INVALIDATED with a go/no-go recommendation per integration. Operator reviews before Phase 1.

### Phase 1 — Webcam + Screen + Virtual Cam (MVP product)
**Goal:** user can open Zoom, pick "GoPro Webcam Studio", see composited GoPro + screen.
**Cadence:** contracts land first (small, sequential), then implementation fan-out, then integration fan-in.
**Exit gate:** end-to-end test script (T13) passes in a clean Windows VM; a recorded MP4 + a Zoom screenshot prove it.

### Phase 2 — Light NLE
**Goal:** record, trim, add text/title, export MP4.
**Cadence:** importer → timeline model → operations → transitions → text → exporter, mostly serial because each builds on the previous contract.
**Exit gate:** a scripted scenario (record 30s, trim to 15s, add title, export) produces a valid 1080p30 MP4 of correct duration.

### Phase 3 — Productize
**Goal:** signed installer, no external deps, ships to a non-technical user.
**Exit gate:** a fresh Windows 11 VM installs the build, plugs in a Hero 8, and the webcam appears in Zoom with no manual steps beyond Next→Next→Finish.

---

## 5. Task graph (what gets spawned, in what order)

Notation: `[P]` = parallelizable, `[S]` = serial, `[G]` = human gate. `backend: cc|codex` = recommended coding agent.

### Phase 0 — Spikes

```
T0-A ──┐
T0-B ──┼──▶ T0-SYN ──▶ [G] Phase 0 review
T0-C ──┘
```

| ID | Title | Backend | Deps | Verifiable acceptance |
|----|-------|---------|------|-----------------------|
| **T0-A** | **Spike: Tauri 2.0 + WebView2 `getDisplayMedia` on Windows** — minimal Tauri app that opens a window, calls `getDisplayMedia`, renders the screen stream to a `<canvas>`, and hardcodes a frame-counter overlay. Throwaway. | codex (greenfield) | none | `cargo tauri dev` boots; a 10s screen recording shows the canvas updating at ≥25 fps. Submit a `spikes/T0-A/recording.mp4` + `verdict.md`. |
| **T0-B** | **Spike: Enumerate & capture GoPro Hero 8 as UVC over USB on Windows** — Rust binary using `nusb` (or `rusb`) to list USB devices, identify the Hero 8 by VID:PID, open the UVC interface, and write 100 raw frames to disk as PNGs. | codex | none | `cargo run --release` prints `GoPro Hero 8 detected` and writes `spikes/T0-B/frame_NNN.png` × 100 with file sizes > 0. |
| **T0-C** | **Spike: DirectShow virtual camera filter skeleton** — minimal C++ DirectShow source filter that registers as a webcam, reads frames from a named pipe, and serves them. No compositor; just "any test pattern flows to Zoom". | cc | none | After `regsvr32`, the filter appears in `ffmpeg -list_devices true -f dshow -i dummy` as `GoPro Webcam Studio`. A 5s mp4 recorded from the filter shows the test pattern. |
| **T0-SYN** | **Spike synthesis + go/no-go** — read verdicts from T0-A/B/C, produce a single `spikes/SYNTHESIS.md` with VALIDATED/PARTIAL/INVALIDATED per integration and a recommendation. | orchestrator (this agent) | T0-A,B,C | `spikes/SYNTHESIS.md` exists and contains one verdict line per spike. |
| **[G]** | Operator reviews the synthesis; confirms Phase 1 architecture (Tauri vs Electron fallback, filter vs OBS-virtcam dependency). | — | T0-SYN | human ack |

### Phase 1 — MVP

```
T1 (repo) ──▶ T2 (CI) ──┬─▶ T3 (SourceBus) ──┐
                        │                     ├─▶ T5 (compositor) ──┐
                        ├─▶ T4 (ScreenCap) ───┘                      │
                        │                                              ├─▶ T9 (scene UI) ──┐
                        ├─▶ T7 (GoPro source adapter) ────────────────┤                    │
                        │                                              ├─▶ T11 (preview) ─┤
                        └─▶ T8 (DirectShow filter, productionized) ───┘                    │
                                                                                             ├─▶ T13 (e2e test)
                                                                                             │
                                                                        T10 (virtualcam pipe) ┘
                                                                        T12 (local recording)
```

| ID | Title | Backend | Deps | Verifiable acceptance |
|----|-------|---------|------|-----------------------|
| **T1** | **Repo + Tauri skeleton + AGENTS.md** — create the repo, Tauri 2.0 workspace with `src-tauri/` (Rust) + `ui/` (React+TS), and an `AGENTS.md` that documents the contract layout for downstream agents. | codex | T0-SYN | `cargo tauri dev` boots a blank window; `cargo test` passes 0 tests; `AGENTS.md` exists. |
| **T2** | **CI: cargo test + lint + tauri build on Windows** — GitHub Actions (or local `cronjob`) that builds the Rust core + UI on every push. Failing builds block merge. | cc | T1 | A push to `main` triggers the run and exits 0 on a clean tree. |
| **T3** | **Contract: `SourceBus` trait + frame type + fake source** — define the trait and a `FakeSource` that emits a color-bar pattern at 30 fps. | cc | T1 | `cargo test source_bus::tests` passes; the test asserts 60 frames in ~2s. |
| **T4** | **`ScreenCaptureSource: SourceBus`** — adapt WebView2 `getDisplayMedia` into the Rust `SourceBus`. Uses Tauri IPC to forward frames from the webview → Rust. | cc | T3, T0-A | `cargo test screen_capture::tests` runs a 5s capture and asserts ≥ 120 frames. |
| **T5** | **Compositor + `Scene` schema** — implement the compositor that takes a `Scene` JSON + active source frames → a single composited `Frame`. Supports: fullscreen, side-by-side, picture-in-picture. | cc | T3 | `cargo test compositor::tests` — given 2 fake sources + a PiP scene, produces a 1920×1080 frame with the small frame inset. Snapshot equality vs golden png. |
| **T6** | **Contract lock-down** — given T3 + T5 exist, freeze their public APIs in `AGENTS.md` (signatures, JSON schemas). | orchestrator | T3,T5 | `AGENTS.md` updated with verbatim signatures; `cargo doc --no-deps` builds without warnings. |
| **T7** | **`GoProSource: SourceBus`** — wraps the T0-B spike's UVC capture into a `SourceBus` impl. Handles disconnect/reconnect events. | cc | T3, T0-B, T6 | `cargo test gopro_source::tests` (skipped when no GoPro connected; runs in CI with a stub). Manually: plug Hero 8, `cargo run --example preview_gopro` shows frames for 10s. |
| **T8** | **Productionize DirectShow filter from T0-C** — adopt the spike's filter, hook it to the named-pipe protocol (T10), add installer registration. | cc | T0-C, T10 (interface) | `regsvr32` registers the filter; the e2e harness in T13 can find it by name. |
| **T9** | **Scene UI (React)** — UI to pick sources, choose a scene template, drag to position PiP. Emits `Scene` JSON over Tauri IPC to Rust. | codex (UI-heavy) | T6 | UI loads; choosing "PiP" and clicking save writes a `Scene` JSON that round-trips through `serde_json::from_str` without error. |
| **T10** | **`VirtualCamPipe` + writer in Rust core** — named-pipe server in the app process that pushes compositor output. Documents the wire protocol in `AGENTS.md`. | cc | T6 | `cargo test virtual_cam::tests` — writer pushes 100 frames; a mock consumer reads them with matching headers. |
| **T11** | **Preview canvas (WebView2)** — render the compositor output to a `<canvas>` at ≥30 fps via Tauri IPC streaming. | cc | T5,T9 | Manual: launching the app shows a live canvas. Automated: `cargo test preview_latency` measures inter-frame delta, p99 < 50ms over 10s. |
| **T12** | **Local recording (MP4 via FFmpeg)** — encode compositor output to disk while streaming to virtual cam. | cc | T6,T10 | `cargo test recording::tests` records 5s and produces a valid MP4 (ffprobe exits 0, duration 5±0.2s). |
| **T13** | **E2E test: open Zoom → see composited feed** — Windows VM script that launches the app, picks PiP, starts streaming, runs a fake-Zoom DirectShow consumer, captures 10s, and asserts the consumer received ≥ 250 frames. | cc | T8,T10,T11,T12 | Script exits 0; `e2e_output.mp4` exists and is 10±1s long. |
| **[G]** | Operator reviews T13 recording; approves Phase 2. | — | T13 | human ack |

### Phase 2 — Light NLE

```
T14 (Project schema) ─▶ T15 (importer) ─▶ T16 (timeline model) ─┬─▶ T17 (ops: trim/split/move)
                                                                ├─▶ T18 (transitions)
                                                                ├─▶ T19 (text overlay)
                                                                └─▶ T20 (exporter)
                                                                                         └─▶ T21 (integration scenario)
```

| ID | Title | Backend | Deps | Verifiable acceptance |
|----|-------|---------|------|-----------------------|
| **T14** | **Contract: `Project` timeline JSON schema** — clips, tracks, transitions, text, export settings. Frozen in `AGENTS.md`. | orchestrator ( drafting) + cc (serde impl) | T13 | `cargo test project_schema::tests` round-trips a sample project; JSON schema in `docs/project.schema.json` validates. |
| **T15** | **Importer: MP4/MOV → decoded frame streams** — wraps FFmpeg (via `ffmpeg-next`) to decode a file into a `SourceBus`-compatible frame iterator. | cc | T14 | `cargo test importer::tests` loads a 5s `spikes/T12/sample.mp4`, asserts ≥ 120 frames decoded. |
| **T16** | **Timeline model in Rust** — tracks, clips, timeline resolution (which frames are visible at time T). Pure data; no UI. | cc | T14 | `cargo test timeline::tests` — given a project with 2 overlapping clips, `resolve_at(t)` returns the correct clip at 10 sampled timestamps. |
| **T17** | **Timeline operations** — trim, split, razor, move, snap. Pure functions that take `Project` → `Project`. | cc | T16 | `cargo test ops::tests` — split a clip at T, assert 2 clips with correct durations. |
| **T18** | **Transitions: crossfade, dip-to-black** — rendered by the compositor when the timeline says so. | cc | T16,T5 | `cargo test transitions::tests` — a 2-clip timeline with a 500ms crossfade renders 15 frames at the overlap with alpha gradient. |
| **T19** | **Text overlay** — font, size, color, position, duration. Rendered by compositor. | cc | T16,T5 | `cargo test text_overlay::tests` — renders "Hello" on a 1080p frame; golden png comparison. |
| **T20** | **Exporter** — given a `Project`, render → encode to MP4 (H.264 1080p30/60, 720p30 presets). Uses the compositor + FFmpeg mux. | cc | T15..T19 | `cargo test exporter::tests` — exports a 3-clip project at 1080p30, asserts ffprobe duration ±0.2s and codec == h264. |
| **T21** | **NLE integration scenario** — scripted: record 30s via T12, import, trim to 15s, add a 2s title card, export. | cc | T20 | Scripted test passes; output mp4 is 17s ±0.5s. |
| **[G]** | Operator reviews the exported mp4; approves Phase 3 productization. | — | T21 | human ack |

### Phase 3 — Productize

| ID | Title | Backend | Deps | Verifiable acceptance |
|----|-------|---------|------|-----------------------|
| **T22** | **Replace OBS-virtcam fallback with bundled, signed DirectShow filter** — the T8 filter becomes a proper installer-registered driver; WHQL or attestation-signed. Requires the EV cert from the operator. | cc + manual signing step | T8, T13 | A fresh VM (no OBS installed) sees "GoPro Webcam Studio" as a webcam after install. |
| **T23** | **Installer (NSIS via Tauri)** — bundles Rust binary + UI + filter + ffmpeg. Auto-update via Tauri updater. | cc | T22 | VM install: Next→Next→Finish. App launches; "Check for updates" works against a test feed. |
| **T24** | **Firmware-update prompt for Hero 8** — detect old firmware, link to GoPro's utility. | cc | T7 | `cargo test firmware_check::tests` with a stubbed USB descriptor. |
| **T25** | **Telemetry + crash reporter (opt-in)** — Sentry, opt-in dialog on first run. | cc | T23 | Crashing the app on a test build produces a Sentry event in the test project. |
| **T26** | **Fresh-VM acceptance** — scripted: install build on a clean Windows 11 VM, plug Hero 8, launch app, verify webcam appears in Zoom within 60s. | orchestrator | T22–T25 | Script exits 0; a recorded video shows Zoom with the GoPro feed. |

---

## 6. Per-task agent brief template

Every task spawns an agent with a self-contained prompt. The orchestrator fills this template per task; the agents never see the broader plan, only their brief.

```
TASK: <title>
REPO: <absolute path on this machine>
WORKDIR: <repo path>
BACKEND: claude-code | codex   (and the exact CLI invocation, below)
BUDGET: --max-turns N  (claude)  |  --full-auto  (codex)

GOAL (one sentence):
<what this task accomplishes>

CONTRACTS YOU MUST OBEY (read before writing code):
- <verbatim Rust trait or JSON schema, or pointer to AGENTS.md section>
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
- modify any contract in AGENTS.md (those are orchestrator-owned)
- introduce dependencies not in Cargo.toml / package.json without <flag>

VERIFY YOURSELF BEFORE EXITING:
- Run the acceptance predicate. Paste the exact output into your final
  message. If it fails, you have not completed the task.

COMMIT:
- One commit, message: "<type>: <task-id> <one-line>".
- Push only if instructed.
```

**Two-stage review (per task, after the coding agent exits):**

1. **Spec-compliance review (orchestrator runs the acceptance predicate).** If it fails, the task is not done — re-spawn the agent with the failure output appended to the brief.
2. **Code-quality review (separate Claude Code read-only run).** `claude -p 'Review the diff in <repo> for: contract violations, unsafe Rust without justification, unbounded dependencies, obvious perf cliffs. Output PASS or FAIL with reasons.' --allowedTools 'Read,Bash(git diff *)' --max-turns 6`. FAIL → spawn a fix task.

Only then does the orchestrator mark the kanban card complete.

---

## 7. Orchestration mechanics (how the operator drives this)

### Option A — kanban board (recommended for the full build)

1. **One-time setup:** `hermes kanban init` (or via `kanban_create`). The board is the durable source of truth — survives `/reset`, survives restarts.
2. **Per phase:** orchestrator calls `kanban_create` with the task briefs, `assignee` set to the appropriate profile (e.g. `claude-code-runner`, `codex-runner`, or just the default Hermes profile), and `parents=[...]` for the dependency graph. Independent tasks have no parent links → dispatcher fans them out.
3. **Workers:** each dispatched worker is a fresh coding-agent run (claude `-p` or codex `exec`) launched via terminal in the worker's isolated session, with `HERMES_KANBAN_TASK` pinned so the worker sees only its card.
4. **Verifiers:** the acceptance predicate runs as a separate, read-only coding-agent invocation. Its output feeds `kanban_complete` or `kanban_block`.
5. **Operator visibility:** `hermes kanban tail <id>` in real time; `hermes kanban show <id>` for the full drawer; ⚠ badges flag stuck workers; `hermes kanban reclaim` for fast recovery.

### Option B — delegate_task batch (lighter, no durability)

For Phase 0 spikes (3 independent leaf tasks + 1 synthesis), use:
```
delegate_task(tasks=[
  {"goal": "<T0-A brief>", "toolsets": ["terminal","file","web"], "role":"leaf"},
  {"goal": "<T0-B brief>", "toolsets": ["terminal","file","web"], "role":"leaf"},
  {"goal": "<T0-C brief>", "toolsets": ["terminal","file","web"], "role":"leaf"},
])
```
Then a second `delegate_task` for T0-SYN with the parents' outputs in its `context`. Faster to set up; no SQLite persistence — good enough for Phase 0, not for Phases 1-3.

### Option C — Claude Code `--agents` + worktrees (for parallel implementation in Phase 1)

When contracts are locked (post-T6), the implementation fan-out can run as parallel Claude Code worktree sessions:
```
# Three independent implementation tasks in parallel worktrees
git worktree add ../gopro-t7  -b t7-gopro-source
git worktree add ../gopro-t9  -b t9-scene-ui
git worktree add ../gopro-t10 -b t10-virtualcam-pipe

# Each worktree gets a Claude Code session
claude -w t7  --tmux -p "<T7 brief with paths in this worktree>"
claude -w t9  --tmux -p "<T9 brief>"
claude -w t10 --tmux -p "<T10 brief>"
```
A later merge task reconciles the worktrees. **Caveat (from the claude-code skill):** monitor each with `tmux capture-pane`, watch for the `❯` prompt (waiting for input), and clean up sessions when done.

### Nightly verification (durable, via cronjob)

```
cronjob(action="create", schedule="0 3 * * *",
        name="gopro-studio-nightly",
        prompt="Pull main, run `cargo test --workspace` and the T13 e2e script in the gopro-studio repo. Report PASS/FAIL per suite. If any FAIL, list the failing test names.",
        workdir="<repo path>",
        enabled_toolsets=["terminal","file"])
```
Delivers a nightly build-health ping; failures escalate to the operator.

---

## 8. Risks & mitigations specific to the agentic build

| Risk | Mitigation |
|---|---|
| **Agent hallucinated an API that doesn't exist** (e.g. a Tauri API that's not in 2.0) | T0-A spike runs first; acceptance predicate is a *running* app, not a stub. If the spike fails, the API assumption is dead before any task relies on it. |
| **Agent "fixes" tests to make them green** | Two-stage review: spec-compliance (run predicate in a fresh checkout) + code-quality review (`git diff` only). Red test → red; making it green is the next task, not a test edit. |
| **Contract drift between agents** | T6 freezes contracts in `AGENTS.md`. Any later task that wants to change a contract opens a *contract-change* task assigned to the orchestrator, not to itself. |
| **Agent stuck in a loop** (30 turns, no progress) | `--max-turns N` caps every invocation. Kanban workers auto-block after `failure_limit` (default 2). Operator sees ⚠ badge. |
| **Quiet unverified "done"** | Acceptance predicate is mandatory and machine-run. A worker that asserts "done" without the predicate passing is automatically re-spawned with the failure output. |
| **Spike gets promoted to production by accident** | Spike tasks live in `spikes/<id>/`; production tasks must explicitly *adopt* files (T7 imports from `spikes/T0-B/src/uvc.rs` — the path is in the brief, but the spike stays put). |
| **Driver signing bricks Phase 3** | EV cert is a human gate before T22. The orchestrator pauses Phase 3 until the operator confirms the cert is in hand. |
| **Cost overrun** | Phase 0 + 1 is ~25 tasks; at ~$0.10–$0.50 per coding-agent run, the all-in agent cost for v1 is on the order of $50–$150 in API spend. Track via `--max-budget-usd` on Claude Code; `claude --max-budget-usd 2` per task as a sane ceiling. |

---

## 9. Open questions for the operator (gating decisions)

Same five from the original scope, plus one agentic-build-specific one:

1. **Windows-only, or macOS too?** Scoped above as Windows-only.
2. **NLE in v1, or Phase 2?** I recommend Phase 2 (gets MVP to you ~6 weeks sooner in wall-clock, much sooner in agent-budget terms).
3. **Virtual cam: OBS-Virtual-Camera detection (Phase 1a, faster) vs our own filter from day one (Phase 1b, +3 wks)?** Scoped above as bundled filter (T8), since the agentic cost of the extra C++ is low.
4. **Wi-Fi RTMP (untethered GoPro) — needed, or Phase 3 stretch?**
5. **Record GoPro + screen simultaneously, or one source per recording?**
6. **NEW: Do you have Claude Code (`@anthropic-ai/claude-code`) and/or Codex (`@openai/codex`) installed on this Windows box?** The per-task briefs assume at least one. If neither, the orchestrator falls back to `delegate_task` with Hermes's own tool-calling model — slower, smaller context, but workable.

---

## 10. What's next

Answer the 6 gating questions. Then I will:

1. Set up the kanban board (or `delegate_task` batch for Phase 0).
2. Spawn T0-A, T0-B, T0-C in parallel as isolated coding agents.
3. Run T0-SYN (synthesis) when all three return.
4. Pause at the human gate with a single `spikes/SYNTHESIS.md` for your review.

If we go the kanban route, you'll be able to watch progress via `hermes kanban tail` on any task; I'll keep the board clean and intervene only on the explicit gates. Total wall-clock to Phase 0 done: ~2–6 hours of agent run-time (your wall-clock is however long it takes you to come back and ack).
