# Subsystems: Core & Process Architecture

This document contains detailed architecture specifications for **Browser Core, Process Model, Threading, IPC, GPUI, and Navigation** (§6–§11) of the Soul Browser Engine.
For the main architecture index and milestone status, see [`docs/architecture-plan.md`](file:///d:/Hobby/soul/docs/architecture-plan.md).

---

## 6. Process Model

| Stage | Processes | Rationale |
|---|---|---|
| M0–M13 (MVP → early rendering) | 1 process, N threads/tasks | Eliminates IPC and sandboxing as blockers while the actual rendering pipeline is unproven. Crate APIs are still message-shaped. |
| M14 (GPU split) | Browser + GPU | Isolates driver crashes/GPU resets from the UI process — the single highest-value split for stability per unit effort. |
| M15 (Network split) | + Network process | Isolates TLS/parsing of untrusted network data from the browser process; also enables connection reuse across tabs cleanly. |
| M16 (Renderer split, coarse) | + 1 renderer process per **window**, not per origin | Real crash isolation ("this tab died, not the browser") without attempting site isolation. |
| Later / Production-only | Renderer per origin (site isolation) | Explicitly deferred — see §11 and §19. Enormous complexity (out-of-process iframes, cross-process postMessage, process reuse policy) for marginal MVP value. |

Each process boundary is introduced **only after** the thing it isolates already works correctly in-process. Do not sandbox a renderer that doesn't render.

---

## 7. Threading Model

- **UI thread** — owned by GPUI. Never blocks; never touches network or does layout directly. Only reads/writes committed layout/paint results and issues commands.
- **Core thread** — owns tab/session/navigation state machines; single-writer to avoid lock contention on shared state.
- **Renderer thread(s)** — one per active tab (or per window in Phase 1), running HTML parse → style → layout → paint. Background/frozen tabs' renderer threads are parked (see Tab lifecycle, §9).
- **JS thread** — co-located with renderer thread per tab initially (JS and DOM interleave heavily; splitting them adds IPC cost with no MVP benefit). Split out only if profiling shows main-thread JS blocking layout unacceptably.
- **Compositor thread** — receives display lists, rasterizes/uploads to GPU, independent of renderer thread so scrolling/compositing stays smooth even if a renderer thread is busy (the single most important thread split for perceived performance).
- **Network runtime (tokio multi-threaded)** — a small pool (2–4 threads) dedicated to async IO; never shares threads with rendering.
- **IO/disk thread(s)** — SQLite access, cache reads/writes, image decode (CPU-bound, pool via `rayon` or a bounded tokio blocking pool).
- **GPU thread** — owned by the platform GPU backend (wgpu's internal submission thread + our own frame-scheduling thread later, once a GPU process exists).

---

## 8. IPC Architecture

**Phase 1 (in-process):** typed command/event enums over `tokio::sync::mpsc` (async boundaries: UI↔core, core↔network) and `crossbeam-channel` (sync boundaries: renderer↔compositor display-list handoff, which must be lock-free and low-latency per frame). No serialization — these are Rust values moved across a channel.

**Phase 2 (cross-process):** the same command/event enums are serialized. Recommended stack:
- Transport: Windows named pipes (`\\.\pipe\...`) via the `interprocess` crate, or raw Win32 `CreateNamedPipe`/`ConnectNamedPipe` through the `windows` crate if `interprocess` proves insufficient for duplex + multiple clients.
- Framing: length-prefixed frames (u32 LE length + payload), one connection per process pair.
- Serialization: `rkyv` (zero-copy deserialization, matters for per-frame display-list traffic) for hot paths (compositor traffic), `postcard` (compact, serde-based, simpler) for low-frequency control messages (navigation, downloads). Avoid `bincode`'s version-fragility for anything crossing a process boundary that will outlive a single build.
- Validation: every message received across a process boundary is treated as **untrusted input** and validated before use (bounds checks, enum discriminant checks) — this is a real security boundary once renderer processes are sandboxed, not just a convenience API.
- Shared GPU memory: display-list *pixels* don't go through the pipe — the renderer/compositor hands the GPU process a shared DXGI texture handle; only small metadata (damage rects, texture handle, frame ID) crosses IPC. Full display lists cross IPC only when the renderer itself is out-of-process from the compositor.

---

## 9. GPUI Architecture

GPUI (from Zed) is a real, GPU-accelerated retained/immediate hybrid UI framework: on Windows it currently renders its own UI via a Direct3D backend (a community fork, `gpui-ce`/`gpui-wgpu`, swaps this for `wgpu` for a unified cross-platform backend — worth evaluating specifically because it would let the browser's own compositor and GPUI's UI renderer share **one wgpu device**, avoiding cross-API texture interop). GPUI exposes a `Surface` element specifically meant for embedding externally-rendered content (video, GPU surfaces) inside its element tree — this is the integration point for web content.

**Boundary:**

```text
GPUI                              (browser UI: tabs, omnibox, toolbar, menus, settings)
  └─ Surface element               (one per visible tab's viewport)
        ▲
        │ texture handle, updated per composited frame
        │
compositor (wgpu)                 (owns the "page" pixels)
        ▲
        │ display list
        │
renderer (layout/paint)           (owns DOM/CSSOM/layout tree)
```

- GPUI **never** receives DOM nodes, layout boxes, or CSS. It receives a texture and forwards raw/translated input events (mouse, keyboard, wheel) that land inside the `Surface` bounds.
- The renderer/compositor **never** draws the window UI, and has no knowledge of GPUI's element tree.
- If `gpui-ce`'s wgpu backend is adopted, the compositor renders directly into a wgpu texture registered with GPUI's `Surface`, avoiding a DXGI shared-handle round trip. If mainline GPUI's native D3D11 backend on Windows is used instead, a DXGI keyed-mutex shared texture is the interop path. **This is an explicit ADR decision point (see §30, ADR-1) to make before M1**, because it affects the compositor's device creation code.

### Input routing

```text
Win32 (WM_MOUSEMOVE, WM_KEYDOWN, WM_MOUSEWHEEL, WM_TOUCH...)
   ↓
GPUI platform layer (translates to GPUI InputEvent)
   ↓
GPUI element tree dispatch (browser-UI elements get first refusal — e.g. clicking the omnibox)
   ↓
Surface element (if event falls inside the active tab's viewport and the browser UI didn't consume it)
   ↓
Input Router (soul-core) — translates GPUI coordinates → page (CSS pixel) coordinates, accounts for zoom/DPI/scroll offset
   ↓
Hit Testing (renderer) — walks the paint/layout tree's hit-test data structure (not the raw DOM — a spatial index built during layout)
   ↓
DOM event dispatch (capture → target → bubble) → JS event listeners
```

Menus, context menus, notifications, downloads UI, history, and bookmarks UI are **entirely GPUI views** backed by `soul-core` state (they read/write history/bookmarks/downloads through the same command/event API the renderer uses — no special-cased path).

### Isolating the GPUI dependency (amendment)

GPUI is a comparatively young, still-fast-moving framework, and a fork of it (`gpui-ce`) more so. Betting the entire browser-UI layer — tabs, omnibox, menus, input routing — directly on it is a **project-survival risk**, not just a rendering-backend choice: an upstream breaking change, a stalled fork, or an API surface that doesn't cover a needed capability can stall the whole project, not just the UI polish.

The mitigation costs little and pays for itself the first time GPUI's API shifts: define a `SoulBackend` trait in `soul-ui` (window/view lifecycle, input event delivery, surface/texture embedding, basic layout primitives for the browser UI) and put all direct `gpui::*` usage behind a `soul-backend-gpui` implementation crate. `soul-core` and `compositor` depend only on the trait, never on `gpui` directly. If GPUI becomes untenable, the fallback path is "write a new backend crate against `egui`, `slint`, or raw Win32," not "rewrite the browser." This is the same message-shaped-API discipline already applied to IPC (§8, ADR-5) applied one layer up, to the UI framework itself.

---

## 10. Browser Core

Owns everything that isn't "render a page" or "fetch bytes":

- **Window manager** — GPUI window lifecycle, multi-window/multi-monitor state, DPI change handling (the user's prior Aura work on WorkerW/DPI virtualization is directly relevant Win32 experience here).
- **Tab manager** — tab creation/close/reorder/pin/mute, and (this project's differentiator, per prior work) a **tiered tab lifecycle**: Active → Background → Frozen, gating renderer thread scheduling and memory retention.
- **Navigation controller** — owns the navigation state machine (§11 diagram), cancellation, redirect handling, races between concurrent navigations in the same tab.
- **Session manager** — window/tab restore across restarts and crashes, serialized on every navigation commit (cheap: URL + scroll offset + form data opt-in, not full DOM snapshots in MVP).
- **Profile manager** — one default profile in MVP; multi-profile and private-browsing profiles are Phase 2 (isolated storage roots, no cross-profile cookie/history leakage — this is a correctness requirement, not a nice-to-have, once it exists at all).
- **Permission manager** — origin-scoped permission store (camera/mic/location/notifications) — stubbed to "always deny" in MVP since there's no JS Web API surface requesting them yet; real implementation lands with the relevant Web APIs.

---

## 11. Navigation System

### States

```text
IDLE → PENDING → (REDIRECTING)* → RESPONSE_RECEIVED → COMMITTED → LOADING → COMPLETE
                                        │
                                        ├─► FAILED (network/TLS/DNS error) → ERROR_PAGE
                                        └─► CANCELLED (user navigated away / stop pressed)
```

- **Navigation races**: each navigation gets a monotonically increasing `NavigationId`. A tab holds only its *current* `NavigationId`; any response arriving for a stale ID is discarded. This single rule eliminates the classic "slow response from an old navigation overwrites the new page" bug class.
- **Back/forward**: a per-tab `Vec<SessionEntry>` + cursor; back/forward re-navigates by replaying the entry (with a cache-preferring fetch) rather than reconstructing DOM state from scratch in MVP (true bfcache — freezing and resuming a live renderer state — is Phase 3+; see feature matrix).
- **New windows / popups / `target="_blank"`**: routed through `soul-core`'s window manager, subject to a popup policy (MVP: allow only user-gesture-triggered `window.open`, deny script-triggered popups without a gesture flag propagated from the JS engine's event dispatch).
- **External URL schemes** (`mailto:`, `tel:`, custom schemes) are handed to `ShellExecute`/`ShellExecuteEx` via Win32, after an explicit allowlist check (see Security, §23 — this is a classic Windows browser CVE class).
- **Crash recovery**: if a renderer thread/process for a tab panics or a GPU device-lost event occurs, the tab is shown an "Aw, Snap"-style error view and can be reloaded independently of other tabs — this requires the M16 process split to be a true guarantee (a panic in an in-process renderer thread in Phase 1 can be caught with `catch_unwind` at the task boundary as a partial mitigation, but a genuine memory-safety violation in `unsafe` FFI code cannot be caught this way, which is itself an argument for prioritizing the process split once `unsafe` surface area grows in M14+).
