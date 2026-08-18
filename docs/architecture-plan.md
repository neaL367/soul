# Building a Browser from Scratch in Rust + GPUI on Windows 11
### A Production-Oriented Engineering Plan

> **Revision note:** this plan was reviewed after its first draft. The review correctly identified the original 12–18 month MVP timeline as optimistic, flagged the GPUI dependency as an under-weighted project-survival risk (not just a rendering-backend choice), argued the JS-engine compatibility spike needed to move earlier, and pushed for an accessibility semantic tree to be carried from early layout work rather than retrofitted. All four are incorporated below, marked inline where they change prior sections. Memory targets were also revised to be more honest about what Rust actually buys (safety, not automatically-low memory).

> **Implementation status note (2026-08-14):** this plan describes the target architecture; §31 now carries a per-milestone **Status** annotation plus a status table that is authoritative for what actually exists in the repository (verified against code, not the changelog). In short: the full M0–M23 milestone sequence exists as working, tested code, with **M1 now genuinely real** — `soul-backend-gpui` opens an actual native window via GPUI (git `zed-industries/zed` gpui 0.2.2 + `longbridge/gpui-component`, both confined to the one backend crate), presenting engine frames as window content; verified by a Win32 `EnumWindows` smoke test. Remaining headless/simulated: M10b GPU compositing (CPU raster only; no surface/DXGI), M14–M16 (single process; IPC and network-service are in-process), and M18 (playback is a state machine, no Media Foundation). Two milestones were renumbered to match the repository's actual sequence (M20 = Downloads, M21 = Compositor + benchmark harness; the original M20 WPT-compatibility milestone is not started — see ADR-16 and §31).

---

## 1. Executive Summary

This is a plan for a real browser engine, not a WebView wrapper. It is written for a **solo-to-small-team developer**, in Rust 1.97.1 / Edition 2024, using **GPUI** for the desktop UI, targeting **Windows 11** first.

The central engineering bet that makes this tractable is: **defer multi-process architecture, keep single-process modularity that is shaped like a future multi-process boundary.** Chromium took hundreds of engineers over a decade to reach site isolation and full sandboxing. A solo developer who tries to build IPC, sandboxing, and crash-isolated multi-process rendering *before* the renderer can lay out a `<div>` will never finish the renderer. Instead, every subsystem is designed behind a message-passing API (commands in, events out) from day one, running in-process on threads/tasks. When the project is mature enough that process isolation is worth the cost, those same APIs move across an OS process boundary with comparatively small changes to call sites, because the *shape* of the interface never changes — only its transport.

This plan is honest about scope: a fully HTML5/CSS3/ES2024-compliant, GPU-accelerated, sandboxed, multi-process browser competitive with Chromium is a **multi-year, likely multi-person** endeavor. What is realistic for one strong systems engineer, working incrementally, is a **usable, GPU-accelerated browser for a well-defined subset of the modern web** (static and moderately dynamic sites, forms, images, basic JS, no video-conferencing-grade media, no extensions), with the architecture below never requiring a rewrite to keep growing after that.

**Timeline, revised:** the original estimate of 12–18 months to MVP (M13) undercounted the hardest stretch of the whole plan — M8 (Layout) through M10 (Compositor), where "fragment geometry is correct" and "pixels are correctly on screen" turn out to be separated by text rendering (subpixel AA, ClearType-matching, font-fallback correctness, DPI-scaled glyph atlases), not just GPU plumbing. Text is also the single thing that most visibly separates a "real" browser from a toy one, so this stretch can't be rushed to hit a date. A realistic MVP timeline for a solo/small-team effort is **24–30 months**, with M10 alone plausibly taking 4–6 months once split (see §31) into a software-raster checkpoint and a GPU-compositor milestone. The architecture doesn't change because of this revision — only the schedule honesty does.

---

## 2. Design Goals

- **Memory safety first.** `unsafe` is isolated to FFI boundaries (Win32, GPU, font/media libraries) and reviewed as a distinct category of code.
- **Incremental usability.** After every milestone, `soul-shell.exe` should build and let you browse *something* real — even if it's one static HTML page with inline CSS.
- **No rewrite architecture.** Crate boundaries are drawn where process boundaries will eventually go (renderer, network, GPU, storage). Internal APIs are message-shaped, not just function-shaped.
- **Reuse for solved problems, build for the differentiator.** HTML tokenization, TLS, and DNS are solved problems — use mature crates. Layout, style cascade tuned to this engine, tab/process lifecycle, and the compositor integration with GPUI are where the actual engineering work is.
- **Explicit phase boundaries.** Every feature is tagged MVP / Phase 2 / Phase 3 / Advanced / Extremely Difficult, and nothing is implemented out of order without a documented reason.
- **Observability from the start.** Structured logging (`tracing`), crash dumps, and a minimal internal `about:` diagnostics surface exist before M5, because debugging a browser with no visibility into its own state is how these projects die.

## 3. Non-Goals (at least through Phase 3)

- Not a Chromium/Firefox competitor on web compatibility. Long tail CSS/JS compatibility is explicitly out of scope for a long time.
- No browser extensions / WebExtensions API.
- No full accessibility tree / screen reader support in MVP (flagged as a real gap, not dismissed — see Security/Production sections).
- No DRM/EME (Widevine, PlayReady) — legally and technically out of reach for an independent project.
- No mobile, no macOS/Linux support in the initial phases (Windows 11 only; GPUI's cross-platform backends are a later opportunity, not a target).
- No enterprise policy engine, no sync service, no telemetry pipeline.
- No JIT compiler for JavaScript until (if ever) an interpreter-only engine is proven to be the actual bottleneck.
- No attempt at Chromium-grade sandboxing (kernel attack-surface reduction, GPU sandboxing) in the MVP — a *reduced* Windows sandbox (Job Objects + restricted tokens + AppContainer where practical) is the realistic target, and this is stated plainly rather than implied to be equivalent to Chromium's.

---

## 4. High-Level Architecture

Two axes define the architecture:

1. **Ownership axis** — GPUI owns the *browser UI* (windows, tabs, omnibox, menus). The engine owns *page content* (DOM, layout, paint, script). GPUI never parses HTML and the engine never draws a tab strip.
2. **Process axis** — starts as **one process, many threads/tasks**, and progressively splits into **browser / GPU / network / renderer(s)** processes as milestones M14–M16 are reached. Crate boundaries are drawn along this axis from the start even while everything runs in one process.

```text
Phase 1 (M0–M13): Single Process, Multi-Threaded
┌─────────────────────────────────────────────────────────────┐
│                         soul-shell.exe                    │
│                                                                │
│  ┌───────────────┐   commands/events   ┌───────────────────┐ │
│  │  GPUI (UI      │◄───────────────────►│  soul-core      │ │
│  │  thread)       │   (in-proc channel) │  (tabs, nav, state) │ │
│  └───────┬────────┘                     └─────────┬─────────┘ │
│          │ Surface texture                         │           │
│          ▼                                         ▼           │
│  ┌───────────────┐                        ┌───────────────────┐│
│  │  compositor    │◄──display lists────────│  renderer          ││
│  │  (wgpu)        │                        │  (html/css/dom/    ││
│  └───────┬────────┘                        │   layout/js)       ││
│          │                                 └─────────┬─────────┘│
│          ▼                                           │          │
│      GPU / DXGI                            ┌─────────▼────────┐ │
│                                             │  networking (tokio)│
│                                             │  (http/tls/dns)    │
│                                             └─────────┬─────────┘│
│                                             ┌─────────▼────────┐ │
│                                             │  storage (sqlite)  │
│                                             └────────────────────┘
└─────────────────────────────────────────────────────────────┘

Phase 2 (M14+): Split Processes
┌───────────────┐   IPC (named pipe +   ┌───────────────┐
│ Browser process│◄──framed protocol)───►│ GPU process    │
│ GPUI + core    │                       │ wgpu/D3D12     │
└──────┬─────────┘                       └────────────────┘
       │ IPC                                     ▲
       ▼                                          │ shared texture (DXGI)
┌───────────────┐   IPC          ┌───────────────┐
│ Renderer proc  │◄──────────────►│ Network process│
│ (per tab, later│                │ HTTP/TLS/DNS   │
│  per origin)   │                └────────────────┘
└────────────────┘
```

The critical property: **the arrows above don't change meaning** between Phase 1 and Phase 2 — only whether the arrow is a Rust channel or a named pipe.

---

## 5. Complete Architecture Diagram

```text
                                   ┌────────────────────────────┐
                                   │        Windows 11 OS        │
                                   │  Win32 / DXGI / MF / DWrite │
                                   └───────────────┬──────────────┘
                                                    │
                     ┌──────────────────────────────┼──────────────────────────────┐
                     │                     BROWSER PROCESS                          │
                     │                                                              │
   ┌─────────────┐   │   ┌───────────────┐     ┌───────────────┐    ┌────────────┐ │
   │ Input        │──►│──►│ GPUI Shell     │────►│ Input Router   │───►│ Hit Test /  │ │
   │ (mouse/kb)   │   │   │ Windows/Tabs/  │     │               │    │ Focus       │ │
   └─────────────┘   │   │ Omnibox/Menus  │     └───────┬───────┘    └──────┬─────┘ │
                     │   └───────┬───────┘             │                    │       │
                     │           │ commands             │ routed input       │       │
                     │           ▼                       ▼                    ▼       │
                     │   ┌────────────────────────────────────────────────────────┐  │
                     │   │                     soul-core                        │  │
                     │   │  Window Mgr │ Tab Mgr │ Navigation │ Session │ Profile   │  │
                     │   │  Permission Mgr │ History │ Bookmarks │ Downloads       │  │
                     │   └──────┬───────────────────────┬───────────────┬─────────┘  │
                     │          │ page commands          │ net requests  │ storage    │
                     │          ▼                       ▼               ▼            │
                     │   ┌───────────────┐     ┌────────────────┐  ┌────────────┐   │
                     │   │  renderer(s)   │     │  networking     │  │  storage    │   │
                     │   │  HTML/CSS/DOM/ │     │  DNS/TCP/QUIC/  │  │  SQLite/    │   │
                     │   │  Layout/JS     │     │  TLS/HTTP1-3    │  │  Cache/Blob │   │
                     │   └───────┬───────┘     └────────────────┘  └────────────┘   │
                     │           │ display list                                       │
                     │           ▼                                                    │
                     │   ┌───────────────┐                                            │
                     │   │  compositor    │                                            │
                     │   │  (wgpu)        │                                            │
                     │   └───────┬───────┘                                            │
                     │           │ shared surface texture                              │
                     └───────────┼──────────────────────────────────────────────────┘
                                 ▼
                        ┌────────────────┐
                        │  GPU process /   │
                        │  in-proc wgpu    │
                        │  device (Vulkan  │
                        │  or D3D12)       │
                        └────────┬────────┘
                                 ▼
                          DXGI Swapchain → Window
```

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

## 7. Threading Model

- **UI thread** — owned by GPUI. Never blocks; never touches network or does layout directly. Only reads/writes committed layout/paint results and issues commands.
- **Core thread** — owns tab/session/navigation state machines; single-writer to avoid lock contention on shared state.
- **Renderer thread(s)** — one per active tab (or per window in Phase 1), running HTML parse → style → layout → paint. Background/frozen tabs' renderer threads are parked (see Tab lifecycle, §9).
- **JS thread** — co-located with renderer thread per tab initially (JS and DOM interleave heavily; splitting them adds IPC cost with no MVP benefit). Split out only if profiling shows main-thread JS blocking layout unacceptably.
- **Compositor thread** — receives display lists, rasterizes/uploads to GPU, independent of renderer thread so scrolling/compositing stays smooth even if a renderer thread is busy (the single most important thread split for perceived performance).
- **Network runtime (tokio multi-threaded)** — a small pool (2–4 threads) dedicated to async IO; never shares threads with rendering.
- **IO/disk thread(s)** — SQLite access, cache reads/writes, image decode (CPU-bound, pool via `rayon` or a bounded tokio blocking pool).
- **GPU thread** — owned by the platform GPU backend (wgpu's internal submission thread + our own frame-scheduling thread later, once a GPU process exists).

## 8. IPC Architecture

**Phase 1 (in-process):** typed command/event enums over `tokio::sync::mpsc` (async boundaries: UI↔core, core↔network) and `crossbeam-channel` (sync boundaries: renderer↔compositor display-list handoff, which must be lock-free and low-latency per frame). No serialization — these are Rust values moved across a channel.

**Phase 2 (cross-process):** the same command/event enums are serialized. Recommended stack:
- Transport: Windows named pipes (`\\.\pipe\...`) via the `interprocess` crate, or raw Win32 `CreateNamedPipe`/`ConnectNamedPipe` through the `windows` crate if `interprocess` proves insufficient for duplex + multiple clients.
- Framing: length-prefixed frames (u32 LE length + payload), one connection per process pair.
- Serialization: `rkyv` (zero-copy deserialization, matters for per-frame display-list traffic) for hot paths (compositor traffic), `postcard` (compact, serde-based, simpler) for low-frequency control messages (navigation, downloads). Avoid `bincode`'s version-fragility for anything crossing a process boundary that will outlive a single build.
- Validation: every message received across a process boundary is treated as **untrusted input** and validated before use (bounds checks, enum discriminant checks) — this is a real security boundary once renderer processes are sandboxed, not just a convenience API.
- Shared GPU memory: display-list *pixels* don't go through the pipe — the renderer/compositor hands the GPU process a shared DXGI texture handle; only small metadata (damage rects, texture handle, frame ID) crosses IPC. Full display lists cross IPC only when the renderer itself is out-of-process from the compositor.

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
- If `gpui-ce`'s wgpu backend is adopted, the compositor renders directly into a wgpu texture registered with GPUI's `Surface`, avoiding a DXGI shared-handle round trip. If mainline GPUI's native D3D11 backend on Windows is used instead, a DXGI keyed-mutex shared texture is the interop path. **This is an explicit ADR decision point (see §18, ADR-1) to make before M1**, because it affects the compositor's device creation code.

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
- **Tab manager** — tab creation/close/reorder/pin/mute, and (this project's differentiator, per prior work) a **tiered tab lifecycle**: Active → Background → Frozen, gating renderer thread scheduling and memory retention. See §9 (Tab Lifecycle) below.
- **Navigation controller** — owns the navigation state machine (§9 diagram), cancellation, redirect handling, races between concurrent navigations in the same tab.
- **Session manager** — window/tab restore across restarts and crashes, serialized on every navigation commit (cheap: URL + scroll offset + form data opt-in, not full DOM snapshots in MVP).
- **Profile manager** — one default profile in MVP; multi-profile and private-browsing profiles are Phase 2 (isolated storage roots, no cross-profile cookie/history leakage — this is a correctness requirement, not a nice-to-have, once it exists at all).
- **Permission manager** — origin-scoped permission store (camera/mic/location/notifications) — stubbed to "always deny" in MVP since there's no JS Web API surface requesting them yet; real implementation lands with the relevant Web APIs.

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
- **External URL schemes** (`mailto:`, `tel:`, custom schemes) are handed to `ShellExecute`/`ShellExecuteEx` via Win32, after an explicit allowlist check (see Security, §13 — this is a classic Windows browser CVE class).
- **Crash recovery**: if a renderer thread/process for a tab panics or a GPU device-lost event occurs, the tab is shown an "Aw, Snap"-style error view and can be reloaded independently of other tabs — this requires the M16 process split to be a true guarantee (a panic in an in-process renderer thread in Phase 1 can be caught with `catch_unwind` at the task boundary as a partial mitigation, but a genuine memory-safety violation in `unsafe` FFI code cannot be caught this way, which is itself an argument for prioritizing the process split once `unsafe` surface area grows in M14+).

---

## 12. HTML Engine

**Decision: reuse `html5ever`** (Servo's HTML5 tokenizer + tree builder), not hand-write the WHATWG tokenization state machine. The HTML5 tokenizer alone is ~80 states with a large number of parse-error recovery edge cases that exist specifically to match real-world broken HTML — this is a solved, thankless problem with no differentiation value.

```text
Bytes → Encoding sniff (BOM/meta charset/HTTP header, via `encoding_rs`)
      → html5ever Tokenizer → html5ever TreeBuilder → DOM (this project's own DOM crate)
```

`html5ever` is decoupled from any particular DOM implementation via its `TreeSink` trait — this project implements `TreeSink` against its own `dom` crate rather than pulling in Servo's full DOM (which is tied to Stylo/layout assumptions this project doesn't share).

**MVP HTML coverage:** parsing, DOM tree construction, `<head>`/`<body>`, text nodes, attributes, `<a>`, `<img>`, `<div>`/`<span>`/block & inline elements, `<table>` (structure only, not full layout initially), `<form>` + basic input types, `<script>`/`<style>` extraction (execution/application handled by JS/CSS subsystems), `<meta charset>`/`<meta viewport>`.

**Phase 2:** `<canvas>` element (backing store, not the 2D API itself yet), `<video>`/`<audio>` elements (element + basic playback via Media Foundation, not MSE), `<iframe>` (same-process, same-origin only), custom elements registry (parse-level support, not full Custom Elements v1 lifecycle).

**Phase 3 / Advanced:** full `<iframe>` isolation, Shadow DOM, `<template>`, declarative Shadow DOM, full table layout algorithm (auto table layout is notoriously fiddly).

## 13. DOM

A from-scratch DOM crate, because layout, style, and JS bindings all need to walk it with different access patterns and this project's DOM has no reason to carry Servo's Stylo-specific baggage.

- **Storage:** arena-allocated (`slotmap` or a hand-rolled generational arena) node pool, not `Rc<RefCell<Node>>` trees — avoids reference-cycle/borrow-checker pain and is dramatically more cache-friendly for layout traversal.
- **Node identity:** `NodeId` (generational index) is the currency passed between HTML parser, style system, layout, and JS bindings — never raw pointers, keeping everything `Send` where needed for future multi-threaded style/layout.
- **Mutation:** DOM mutations (from parsing *or* from JS) go through one mutation API that also records the invalidation needed for style/layout (dirty bits), so "JS calls `appendChild`" and "parser inserts a node" hit the same invalidation path — no special-casing that could get them out of sync.
- **MVP:** element/text/comment/document nodes, attributes, basic tree mutation API (`appendChild`, `removeChild`, `setAttribute`), `querySelector`/`querySelectorAll` (via the `selectors` crate, shared with CSS matching — see §14).
- **Phase 2:** `MutationObserver`-equivalent internal event stream (needed before JS `MutationObserver` API can exist), Shadow-DOM-aware tree walking.
- **Advanced:** full Shadow DOM encapsulation semantics, slot assignment.

## 14. CSS Engine

**Decision: reuse `cssparser` + `selectors`** (both Servo-maintained, both genuinely low-level and reusable independent of Stylo) for tokenizing and selector matching. **Write the cascade, computed-value resolution, and layout tree from scratch**, tuned to this engine's DOM and to `taffy` (see §15) as the box-layout solver.

```text
CSS bytes → cssparser Tokenizer → this project's rule/declaration parser → CSSOM (Stylesheet/Rule/Declaration)
DOM + CSSOM → selector matching (`selectors` crate against this project's DOM `Element` trait)
            → cascade (origin/importance/specificity ordering — hand-written, ~200-400 LoC, not exotic)
            → computed style per element (property table; start with a fixed, exhaustively-enumerated
              property set rather than a fully generic "any CSS property" system — this is the single
              biggest scope-control decision in the CSS engine)
            → style tree (computed style attached to DOM nodes, with inheritance resolved)
```

**MVP CSS:** box model (content/padding/border/margin), `display: block/inline/inline-block/none`, normal flow + floats (basic), `position: static/relative/absolute/fixed`, colors, `font-*`, `text-align`, basic `background`, `border`, simple selectors + combinators (`.class`, `#id`, `tag`, descendant/child), `!important`, basic cascade/specificity, `overflow: visible/hidden/scroll` (scroll mechanics, not scrollbar theming), viewport meta handling.

**Phase 2:** Flexbox (via `taffy`), CSS Grid (via `taffy`), `position: sticky`, z-index/stacking contexts (correctly — this is a common source of subtle bugs), transforms (2D), opacity, basic transitions, media queries (viewport-based), `calc()`, CSS custom properties (`--var`)/`var()`.

**Phase 3:** animations (`@keyframes`), 3D transforms, filters (`blur`, `drop-shadow` — GPU-shader-backed), `clip-path`, container queries, `:has()` (selector matching cost is real — needs a dedicated invalidation strategy, not naive re-matching).

**Advanced / Extremely Difficult:** full CSS Grid subgrid, exotic writing modes (vertical-rl etc.), full text-wrap/hyphenation locale correctness, print CSS/pagination, view transitions, scroll-driven animations, houdini-style custom paint/layout APIs.

---

## 15. Layout Engine

This is the true from-scratch core of the project, alongside the JS engine decision.

```text
Style tree → Box generation (which boxes exist: block/inline/flex/grid/none — this project's code)
           → Layout tree (this project's tree, NOT the DOM — anonymous boxes, run-ins, etc. live here)
           → Constraint resolution:
                - Block layout: hand-written (BFC width/height resolution, margin collapsing)
                - Inline layout: hand-written (line breaking, integrated with text shaping — see §15b)
                - Flexbox/Grid: delegate box-constraint solving to `taffy`'s low-level API
                  (this project implements `taffy::LayoutPartialTree` against its own layout nodes —
                  taffy does NOT own the tree; it computes sizes/positions for a tree we still own,
                  which matters because block/inline/flex/grid boxes have to interoperate in one tree)
           → Fragment tree (positioned boxes with final geometry — input to paint)
```

- **Absolute/fixed positioning**: resolved as a second pass against the nearest positioned containing block, after normal-flow layout of that containing block completes.
- **Stacking contexts / z-index**: computed as a tree parallel to the fragment tree during paint-list generation, not folded into layout — keeps layout invalidation independent of paint-order changes (e.g., an `opacity` animation shouldn't re-run layout).
- **Scrolling**: scroll containers get their own coordinate space; scroll offset is a compositor-thread-owned value updated on input, *not* re-triggering layout on every scroll tick (only triggering re-paint/re-composite) — this is the standard "scrolling shouldn't be a layout operation" rule and is one of the highest-leverage performance decisions in the whole engine.
- **Viewport / fonts / text shaping**: see below.

### Text shaping (own subsystem, reused libraries)

`cosmic-text` (which bundles `rustybuzz` for shaping — a Rust port of HarfBuzz — plus `swash` for glyph rasterization and `fontdb` for font matching/fallback) is the recommended reuse target: writing a correct Unicode line-breaker + bidi + shaping engine from scratch is an "extremely difficult" bucket item on its own and has essentially zero differentiation value for a browser project. System font enumeration on Windows goes through **DirectWrite** (`windows` crate bindings) feeding `fontdb`.

**MVP:** left-to-right Latin text, single font per run, basic line breaking (UAX#14 via a reused crate — do not hand-write UAX#14), no ligature/kerning correctness guarantees beyond what `rustybuzz` gives for free.

**Phase 2:** font fallback chains, `@font-face` (WOFF2 via a reused decoder), basic bidi (UAX#9, via `unicode-bidi` crate), hyphenation off by default.

**Advanced:** vertical writing modes, full complex-script shaping correctness (Arabic/Indic/Thai), hyphenation dictionaries, justified text with proper inter-word/inter-glyph distribution.

### Images / SVG / Canvas

- **Images**: decode via the `image` crate (PNG/JPEG/GIF/BMP; WebP and AVIF need additional crates — `image-webp`, and AVIF decode is genuinely hard, defer). Decoding runs on a background thread pool, never the renderer thread — a large image shouldn't stall layout.
- **SVG**: MVP renders `<img src="*.svg">` as a rasterized image via `resvg`/`usvg` (reused, mature, exactly this use case). Inline `<svg>` with DOM/CSS/JS interaction (SVG as a first-class part of the DOM) is Phase 3+ — genuinely a second rendering model bolted onto the first in real browsers, and honestly hard.
- **Canvas**: the `<canvas>` element ships in Phase 2 (§12); the 2D rendering context API (`fillRect`, paths, `drawImage`, etc.) is a real sub-project — implement against the compositor's own 2D drawing primitives (which the layout/paint system already needs for borders/backgrounds), exposed to JS via Web API bindings. WebGL/WebGPU-in-canvas is Advanced.

---

## 16. Paint System

```text
Fragment tree + stacking context tree
   → Display list builder (a flat, ordered list of draw commands: rects, text runs, images, clips, transforms)
   → (optional) display list is diffed against the previous frame for damage regions
   → handed to the compositor
```

The display list is the **serialization boundary** for the eventual renderer/compositor process split (§8) — it is designed as data (not closures/trait objects) from the start specifically so it can be `rkyv`-serialized later without redesign.

## 17. Compositor

- Built on **`wgpu`** (Vulkan or D3D12 backend on Windows, chosen by `wgpu` at device-creation time — see ADR-6, §18). `wgpu` is a safe, actively maintained, cross-platform GPU abstraction; hand-rolling raw Vulkan or D3D12 command buffer management is a large, security-relevant `unsafe` surface with no payoff versus `wgpu` for this project's needs (2D-heavy compositing, not a game engine).
- **Rasterization**: for MVP, CPU-side software rasterization of paint primitives (via `tiny-skia`, a mature Rust `Skia`-like 2D rasterizer) uploaded as textures is *simpler and faster to ship* than a full GPU-rasterized 2D pipeline (analytic AA, path rendering on GPU is its own research area). GPU-side rasterization (compute-shader path rendering, akin to Pathfinder/Vello) is a Phase 3 performance upgrade, tracked explicitly as a swappable backend behind the same display-list-consumer interface.
- **Compositing** (layer tree → final frame) *is* GPU-side from day one — this is different from rasterization: individual painted tiles are simple textured quads, and quad compositing (with transforms/opacity/clips) is exactly what a GPU excels at and is not hard to get right with `wgpu`.
- **Damage tracking**: dirty-rect accumulation per frame; undamaged tiles are not re-rasterized or re-uploaded.
- **Frame pacing / VSync**: driven by DXGI's `Present` with sync interval, coordinated with GPUI's own present cycle so tab content and the browser UI don't visibly tear relative to each other.
- **High-DPI / multi-monitor**: per-monitor DPI awareness (Per-Monitor-V2) — directly reuses the user's prior DPI-virtualization experience from the Aura project; render targets are sized in physical pixels, CSS pixel↔device pixel conversion happens once, at the layout/paint boundary, not scattered through the codebase.

```text
Data flow:
HTML/CSS → Layout → Display List → tiny-skia raster (CPU, MVP) → wgpu texture upload
        → wgpu compositor (GPU: quads, transforms, opacity, clips) → DXGI swapchain → Window
```

---

## 18. JavaScript Engine

### Comparison

| Approach | Verdict |
|---|---|
| **Write a JS engine from scratch** | Rejected for MVP. A spec-compliant parser + bytecode VM + GC + Promises/async is itself a multi-year project (see Boa's and QuickJS's actual histories). Would consume the entire project budget before any page renders JS. |
| **`rusty_v8` / `deno_core` (V8 bindings)** | Best real-world compatibility and JIT performance, but: C++ dependency (violates "prefer Rust-native" without strong justification), huge binary size, slow build times, and DOM binding layer (V8's embedder API) is nontrivial. Kept as an explicit fallback option (ADR-4) if `boa` proves too slow for real sites — not the starting choice. |
| **`rquickjs` (QuickJS bindings)** | Good compatibility, small footprint, has a bytecode interpreter (no JIT) — but still an FFI boundary to a C engine, undercutting the memory-safety goal. |
| **`boa`** (pure-Rust JS engine) | **Recommended starting point.** No `unsafe` FFI boundary, actively maintained, implements a large and growing subset of ECMA-262, has its own GC (`boa_gc`), embeddable with a defined host-object binding API for DOM integration. Interpreter-only (no JIT) — accepted trade-off; most page-load JS (small event handlers, DOM manipulation, simple app logic) is not JIT-bound in practice. |

**Decision:** start with `boa`, embedded as a library inside the renderer/JS thread, with DOM bindings implemented against `boa`'s host-object trait system. Revisit V8 only if profiling shows JS execution (not layout/paint) as the dominant bottleneck on real target sites — and treat that as a major, explicitly-scoped milestone of its own (a new JS engine is not a drop-in swap once DOM bindings exist against the first one's API shape).

**Amendment: move the compatibility question earlier than "revisit if profiling shows a problem."** The real risk isn't that Boa is slow — it's that Boa can't correctly execute JS that real target sites (even "static" documentation/blog sites) assume as baseline (specific `Array`/`Promise` semantics, event-handling edge cases). Discovering this at M11–M12, after DOM bindings, GC integration, and the event loop are already shaped around Boa's host-object API, makes a pivot equivalent to redoing M11–M13. **A standalone spike — run Boa headless against the actual target-site JS corpus before M6 (before layout work, not after JS work) — is now a prerequisite, not a nice-to-have.** If the spike finds Boa can't handle the target corpus, the V8 alternative is chosen before any binding code is written against Boa's API shape, not after.

```text
JavaScript source
   ↓ boa::Lexer
   ↓ boa::Parser → AST
   ↓ boa bytecode compiler
   ↓ boa VM (interpreter)
   ↓ boa_gc (garbage collector)
   ↓ Web API / DOM bindings (this project's code — the actual work)
   ↓ DOM (this project's crate, via §13's NodeId API)
```

**MVP JS:** running `<script>` (inline + external, synchronous), a hand-written **event loop** (task queue + microtask queue — this is this project's own code, sitting on top of `boa`'s VM, not provided by `boa` itself), `console.log` → devtools/stdout, `setTimeout`/`setInterval`, basic DOM API surface (`querySelector`, `addEventListener`, `classList`, `innerHTML` read/write, basic `fetch` bound to the networking crate).

**Phase 2:** Promises/`async`/`await` (event-loop integration is the hard part, not the language feature — needs correct microtask-vs-macrotask ordering), `MutationObserver`, `history.pushState`, richer DOM API coverage, JSON, basic `localStorage`/`sessionStorage` bindings.

**Phase 3:** Web Workers (a second `boa` VM instance per worker, message-passing via `postMessage`, no shared memory), `IndexedDB` bindings, more complete `fetch`/`Response`/`Request`/`Headers`.

**Advanced / Extremely Difficult:** WebAssembly (a *third* execution engine — `wasmtime` embeds cleanly in Rust and is the right reuse choice, but wiring it into the DOM/JS interop model correctly is real work), `SharedArrayBuffer` + Atomics (has security implications — Spectre-class isolation requirements that real browsers solve with site isolation, which this project has explicitly deferred), Service Workers (needs a persistent background execution model + Cache Storage + fetch interception — a project-sized feature on its own).

---

## 19. Networking Stack

```text
URL → url crate (parsing, per WHATWG URL spec)
    → Proxy resolution (system proxy settings via WinHTTP/registry, or manual config)
    → DNS (hickory-resolver, with its own cache)
    → Connection: TCP (std/tokio) for HTTP/1.1 & HTTP/2, or QUIC (quinn) for HTTP/3
    → TLS (rustls, with rustls-native-certs or webpki-roots for the trust store)
    → HTTP/1.1 (hyper) / HTTP/2 (hyper + h2) / HTTP/3 (h3 + quinn)
    → Redirect handling (bounded hop count, method/body semantics per fetch spec)
    → Decompression (flate2 for gzip/deflate, brotli crate, zstd optional)
    → HTTP cache (storage crate: freshness via Cache-Control/Expires, validation via ETag/Last-Modified)
    → Cookie jar (storage crate; parsed/matched per RFC 6265bis rules — reuse `cookie` crate for parsing,
      write matching/storage logic against this project's storage layer)
    → Response handed to renderer (as a byte stream, so HTML parsing can begin before the full body arrives —
      streaming parse is an MVP requirement, not an optimization, for perceived load performance)
```

**Reuse, do not reinvent:** TLS/crypto (`rustls`, backed by `aws-lc-rs` or `ring` as its crypto provider), DNS resolution, HTTP/1.1/2/3 protocol implementations, QUIC. These are security-critical, extensively fuzzed, and re-implementing them is both a security risk and a waste of the project's differentiation budget. This is a hard rule, not a preference (see §17 dependency policy).

**MVP:** HTTP/1.1 + HTTP/2 over TLS 1.2/1.3, DNS resolution with caching, redirects, gzip/br decompression, basic cookie jar, `Cache-Control` HTTP caching, CORS enforcement for `fetch`/XHR (simple + preflight), Same-Origin Policy enforcement at the fetch layer.

**Phase 2:** HTTP/3 + QUIC, connection pooling tuned per-origin, proxy support (system + manual), CSP parsing/enforcement (`script-src`, `connect-src` at minimum), mixed-content blocking, private-browsing network isolation (separate cookie jar/cache, not just "don't persist").

**Advanced:** HTTP/3 0-RTT resumption correctness, full CSP directive coverage, client certificates, HSTS preload list, DNS-over-HTTPS as a user option.

---

## 20. Storage

| Store | Backing | Notes |
|---|---|---|
| Cookies | SQLite (`rusqlite`) | Matches Chromium's approach; gives ACID + easy expiry queries "for free". |
| History | SQLite | Full-text search on titles/URLs is a `FTS5` virtual table — reused, not built. |
| Bookmarks | SQLite | Simple relational tree (parent_id/order columns). |
| LocalStorage | SQLite (one table, origin-partitioned key/value) | Synchronous API contract from JS is satisfied by an in-memory write-through cache over SQLite. |
| SessionStorage | In-memory only, per tab, per origin | Never touches disk — correctness requirement, not a perf shortcut. |
| IndexedDB | SQLite-backed, Phase 3 | IndexedDB's transactional/versioned-schema semantics map reasonably onto SQLite transactions; this is still a substantial spec surface (cursors, key ranges, indexes). |
| Cache Storage (`fetch` API) | Blob files on disk + SQLite index | Large binary bodies don't belong in SQLite rows. |
| HTTP cache | Blob files on disk + SQLite index | Same pattern as Cache Storage; keyed by request hash. |
| Profiles | Directory-per-profile under `%LOCALAPPDATA%` | Private browsing = an in-memory-only profile variant of the same schema, discarded on window close. |

- **Concurrency**: SQLite in WAL mode, one writer connection per store owned by the storage thread/task, reads via connection pool (`r2d2`/`deadpool` + `rusqlite`).
- **Crash recovery**: WAL mode gives this largely for free at the DB level; session restore (open tabs/URLs) is written incrementally, not just at clean shutdown.
- **Migration/versioning**: `PRAGMA user_version` + an explicit migration table, applied at startup — plan for schema changes from the first shipped schema, not after the fact.
- **Quotas**: per-origin storage quota checks before writes to LocalStorage/IndexedDB/Cache Storage (Phase 2+; MVP can use a single generous global cap).
- **Encryption**: not in MVP scope; cookie/password encryption-at-rest (DPAPI-backed, matching how Chromium/Edge protect data on Windows) is a Production-Definition requirement (§21), not MVP.

---

## 21. GPU Architecture

### Vulkan vs. D3D12 vs. wgpu (see also ADR-6, §18)

| Option | Assessment |
|---|---|
| Raw Vulkan | Full control, but a large `unsafe` surface (manual synchronization, memory management) for a 2D-compositing-dominated workload that doesn't need it. Rejected for the compositor. |
| Raw D3D12 | Same trade-off as Vulkan, plus Windows-only (loses the option of GPUI's other platform backends later). Rejected. |
| **`wgpu`** | Safe Rust API over Vulkan/D3D12/Metal, chooses the best backend per platform, actively maintained, already effectively "blessed" by the GPUI ecosystem (`gpui-ce` uses it). **Recommended** for the compositor; interoperates with GPUI's UI rendering per §9. |
| CPU rendering fallback | Required regardless, for headless/testing and as a last-resort compatibility path (old/broken GPU drivers) — `tiny-skia` (already the MVP rasterizer, §16) doubles as this fallback. |

```text
Layout/Paint → Display List
   → Raster (tiny-skia, CPU, MVP) or GPU raster (Phase 3, compute-shader path rendering)
   → wgpu Texture upload
   → wgpu Compositor pass (quads: transform, opacity, clip → GPU)
   → Shared texture handle → GPUI Surface (browser-UI compositing)
   → DXGI Swapchain → Present → Window
```

- **Device initialization**: one `wgpu::Device`/`Queue` per process (shared across all tabs in Phase 1; per-GPU-process in Phase 2).
- **Queues/command buffers/synchronization**: entirely `wgpu`'s responsibility — this project writes render passes, not raw sync primitives.
- **Textures/buffers**: tile-based texture atlas for rasterized content (avoids one-texture-per-element overhead), streaming uploads via `wgpu`'s staging-buffer APIs.
- **Swapchain/surface creation**: one per GPUI window; tab content is composited *into* that swapchain's frame by the compositor, not into a separate swapchain per tab.
- **VSync/frame pacing**: `PresentMode::Fifo` (VSync) by default; frame scheduling budgets rasterization + compositing to land inside the frame deadline, dropping to "present last complete frame" rather than blocking input on a slow raster.
- **Damage tracking**: only damaged tiles are re-rasterized/re-uploaded per frame (§17).
- **High-DPI / multi-monitor**: render target sizing and DPI scale live at the compositor/window boundary (§17); multi-monitor with differing DPI/refresh rates is handled per-window, not globally assumed.

---

## 22. Windows Platform Layer

All access to the Windows API goes through the official [`windows`](https://github.com/microsoft/windows-rs) crate (Microsoft-maintained, generated bindings — reuse, don't hand-write FFI signatures for Win32).

- **Win32 windowing/input**: owned by GPUI for browser windows; the platform layer crate wraps anything GPUI doesn't already expose (e.g., custom window messages for tab-drag-out-to-new-window gestures).
- **Clipboard**: `windows` crate `Clipboard` APIs, format negotiation for HTML/plain-text copy from rendered pages.
- **File system**: standard `std::fs` plus the Windows file picker (`IFileOpenDialog`/`IFileSaveDialog` via `windows` crate) for downloads/uploads.
- **DPI/scaling**: Per-Monitor-V2 awareness declared in the manifest; directly reuses prior Aura-project DPI-virtualization experience.
- **Accessibility**: UI Automation (UIA) provider implementation for the browser UI (GPUI-level, if/when GPUI exposes UIA hooks) and, separately and much harder, an accessibility tree derived from the DOM/layout tree for page content — flagged honestly as a **major gap** through MVP and Phase 2 (see §29).
- **Notifications**: Windows `ToastNotification` via `windows` crate, for the Notifications Web API (Phase 3) and download-complete notices.
- **Audio/video**: Media Foundation (`windows` crate MF bindings) for `<audio>`/`<video>` element playback — direct reuse of prior Aura-project MF experience. Media Source Extensions (adaptive streaming) is Advanced.
- **Process management**: `CreateProcess` with restricted tokens for renderer processes (M16+), Job Objects for resource limiting/cleanup-on-crash.
- **Sandboxing**: realistic target is Job Objects + restricted access tokens + AppContainer profiles for renderer processes — **not** a claim of Chromium-equivalent sandbox strength (Chromium's Windows sandbox is itself a decade-plus of dedicated engineering). This is stated explicitly in §11/§29 rather than glossed over.

---

## 23. Security Architecture

| Protection | MVP requirement? | Notes |
|---|---|---|
| HTTPS + TLS certificate validation | **Yes, MVP** | Via `rustls` + system/webpki trust store; invalid-cert pages get a real interstitial, not a silent bypass. |
| Same-Origin Policy (fetch/XHR/DOM) | **Yes, MVP** | Enforced at the networking + JS-binding layer; this is foundational, not an add-on. |
| CORS | **Yes, MVP** (simple + preflight) | Needed the moment `fetch` exists. |
| `file://` restrictions | **Yes, MVP** | No `file://` → arbitrary local file read from a `http(s)://` page; local-file navigation itself is allowed but sandboxed identically to any other origin. |
| Dangerous URL scheme handling | **Yes, MVP** | Explicit allowlist before handing a URL to `ShellExecute` (§11) — this is a known real-world browser CVE pattern. |
| Download security (MOTW, extension warnings) | **Yes, MVP-adjacent, land early Phase 2** | Mark-of-the-Web (`Zone.Identifier` ADS) on downloaded files, matching Windows/Edge behavior so downloaded files aren't silently trusted. |
| CSP | Phase 2 | Real value once script injection is a realistic risk (i.e., once the JS engine + DOM bindings are real). |
| Mixed content blocking | Phase 2 | |
| IPC message validation | **Yes, from the first cross-process boundary (M14)** | Every IPC message is untrusted input the moment a second process exists. |
| Process sandboxing (Job Object/token/AppContainer) | Phase 2 (M16) | See §22 — explicitly not Chromium-equivalent. |
| Renderer isolation / Site isolation | **Advanced / Production-only, explicitly deferred** | Real site isolation (cross-origin iframes in separate processes, Spectre-class mitigations) requires the process-per-origin model called out in §6 as a later/production item. Stated plainly: this project will not have Chromium-grade site isolation for a long time, if ever, and this is a known, accepted risk that governs what the browser should and shouldn't be trusted for (e.g., not a target for handling untrusted content alongside sensitive logged-in sessions in the same way Chromium can). |
| Secure storage (DPAPI-encrypted cookies/passwords) | Production-only | See §20. |
| Private browsing | Phase 2 | In-memory-only profile variant (§20). |
| Memory safety | **Yes, continuous, by construction** | The whole point of doing this in Rust; `unsafe` blocks are the only place this can regress, and they're concentrated at well-known FFI boundaries (Win32, GPU, media, font libraries) that get extra review/fuzzing priority. |

---

## 24. Repository Structure

```text
browser/
├── Cargo.toml                  # workspace root
├── Cargo.lock
├── rust-toolchain.toml         # pins 1.97.1, edition 2024
├── crates/
│   ├── soul-shell/          # bin: entry point, wires everything together
│   ├── soul-core/           # tab/window/nav/session/profile/permission state machines
│   ├── soul-ui/             # `SoulBackend` trait + backend-agnostic view logic (tabs, omnibox,
│   │                           #   toolbar, menus, settings, downloads UI) — no direct `gpui` import
│   ├── soul-backend-gpui/    # concrete `SoulBackend` impl against GPUI; the ONLY crate that
│   │                           #   depends on `gpui` directly (see §9 amendment)
│   ├── ipc/                    # command/event message types + Phase-1 channel transport +
│   │                           #   Phase-2 named-pipe/framing transport, behind one trait
│   ├── html/                   # html5ever TreeSink impl → this project's DOM
│   ├── dom/                    # arena-based DOM, NodeId, mutation API, MutationObserver plumbing
│   ├── css/                    # cssparser/selectors integration, CSSOM, cascade, computed style
│   ├── layout/                 # box generation, block/inline layout, taffy integration (flex/grid)
│   ├── text-shaping/           # cosmic-text/rustybuzz/fontdb/DirectWrite integration
│   ├── paint/                  # display list types + builder
│   ├── raster/                 # tiny-skia CPU raster backend (Phase 3: GPU raster backend, same trait)
│   ├── compositor/             # wgpu compositing, tiling, damage tracking, frame scheduling
│   ├── javascript/             # boa embedding, event loop (task/microtask queues), GC integration
│   ├── web-api/                # DOM bindings, fetch/Promise/timers/etc. bound into `javascript`
│   ├── networking/              # url/DNS/TCP/QUIC/TLS/HTTP1-3/cookies/CORS/CSP
│   ├── storage/                # SQLite-backed cookie/history/bookmarks/LocalStorage/cache/profiles
│   ├── image-decode/            # `image`/`resvg` integration, background decode pool
│   ├── media/                  # Media Foundation bindings for <audio>/<video>
│   ├── gpu/                    # wgpu device/surface management shared by compositor + soul-ui
│   ├── platform-windows/        # Win32 wrappers not owned by GPUI: shell execute, MOTW, UIA, notifications
│   ├── sandbox/                # (Phase 2+) Job Objects, restricted tokens, AppContainer setup
│   ├── downloads/               # download manager (networking + storage + platform-windows glue)
│   ├── devtools/                # (Phase 2+) inspector/console/network panel backend + UI
│   └── common/                  # shared small types (Url newtype wrappers, error types, tracing setup)
├── resources/                    # default icons, built-in error pages, default stylesheet (UA stylesheet)
├── tests/                        # workspace-level integration + web-platform-test harness
├── benchmarks/                   # criterion benches: layout, paint, parse
└── docs/                         # this document + ADRs (see §30) + per-crate design notes
```

**Dependency direction** (no cycles, enforced by workspace lint or CI check): `soul-shell` → `soul-backend-gpui` → `soul-ui` (trait only) / `soul-core` → `ipc` → {`html`, `css`, `dom`, `layout`, `javascript`, `networking`, `storage`, `compositor`} → {`gpu`, `text-shaping`, `raster`, `image-decode`, `media`, `platform-windows`} → `common`. `dom`, `css`, and `layout` deliberately do not depend on `networking` or `storage` — they operate on bytes/values already fetched, keeping them unit-testable without any IO. **`gpui` itself appears only in `soul-backend-gpui`'s `Cargo.toml` — no other crate in the workspace, including `soul-core` and `compositor`, depends on it directly** (§9 amendment); this is the concrete enforcement mechanism behind the trait-boundary decision, checkable in CI via `cargo metadata`.

---

## 25. Dependency Strategy

| Subsystem | Decision | Crate(s) |
|---|---|---|
| TLS/crypto | **Reuse — never hand-roll** | `rustls` (+ `aws-lc-rs` or `ring` provider) |
| DNS | Reuse | `hickory-resolver` |
| HTTP/1.1, HTTP/2 | Reuse | `hyper`, `h2` |
| HTTP/3, QUIC | Reuse | `quinn`, `h3` |
| HTML tokenizing/parsing | Reuse | `html5ever` |
| CSS tokenizing, selector matching | Reuse (primitives only) | `cssparser`, `selectors` |
| CSS cascade, computed style, layout tree | **Build from scratch** | — |
| Flexbox/Grid box-constraint solving | Reuse (low-level API, tree owned by us) | `taffy` |
| Block/inline layout, line breaking integration | **Build from scratch** | (uses `unicode-linebreak`/UAX#14 crate for break opportunities) |
| Text shaping/rasterization | Reuse | `cosmic-text` (`rustybuzz` + `swash`), `fontdb` |
| System font enumeration | Provided by Windows | DirectWrite via `windows` crate |
| Image decoding | Reuse | `image`, `image-webp` |
| SVG (as `<img>`) | Reuse | `resvg`/`usvg` |
| 2D CPU rasterization | Reuse | `tiny-skia` |
| GPU abstraction | Reuse | `wgpu` |
| JavaScript engine | Reuse (pure-Rust) | `boa_engine` |
| WebAssembly (Phase 3+) | Reuse | `wasmtime` |
| DOM/JS bindings, event loop, layout engine internals | **Build from scratch** | — differentiator code |
| Compression | Reuse | `flate2`, `brotli` |
| Cookie parsing | Reuse (matching logic ours) | `cookie` |
| Storage/database | Reuse | `rusqlite` (+ `r2d2`/`deadpool`) |
| Async runtime | Reuse | `tokio` (networking/storage/IO); GPUI's own executor for UI-thread tasks |
| IPC transport (Phase 2) | Reuse (transport), build (protocol) | `interprocess`, `rkyv`/`postcard` |
| GPU driver / OS APIs | Provided by platform | Windows/DXGI/DirectX runtime, GPU vendor driver |
| Browser UI framework | Provided (chosen) | `gpui` |

**Policy:** security-critical primitives (crypto, TLS, memory-unsafe parsing of untrusted input where a mature hardened crate exists) are **never** reimplemented without a documented, specific technical reason reviewed as its own ADR. The project's actual novelty budget is spent on: the layout engine, the tab lifecycle/memory-tiering system, the GPUI↔compositor integration, and DOM/JS binding design — not on re-solving TLS or HTML tokenization.

---

## 26. Feature Matrix

Legend: **M**=MVP, **R**=Required (ships shortly after MVP), **L**=Later/Phase 2-3, **A**=Advanced, **X**=Extremely Difficult, **O**=Optional/may never ship.

### Browser Core
| Feature | Class | Feature | Class |
|---|---|---|---|
| Tabs (open/close/switch) | M | Session restore | R |
| Windows (multi-window) | M | Bookmarks | R |
| Navigation (URL bar, back/fwd) | M | Downloads | R |
| Search (omnibox → search engine) | M | Tab freeze/discard (memory tiering) | R |
| History | M | Sync / multi-device | O |

### HTML
| Feature | Class | Feature | Class |
|---|---|---|---|
| Parser (html5ever) | M | Forms (basic inputs) | M |
| DOM tree | M | Tables (structural) | M |
| Links/navigation | M | Custom elements (parse-level) | L |
| Images | M | Shadow DOM | A |
| Metadata (`<meta>`, `<title>`) | M | Full table layout algorithm | A |

### CSS
| Feature | Class | Feature | Class |
|---|---|---|---|
| Selectors (basic + combinators) | M | Grid | L |
| Cascade/specificity | M | Media queries | L |
| Box model | M | Transforms (2D) | L |
| Positioning (static/relative/absolute/fixed) | M | Transitions | L |
| Flexbox | R | Animations (`@keyframes`) | A |
| `:has()`, container queries | A | Print/pagination CSS | X |

### JavaScript
| Feature | Class | Feature | Class |
|---|---|---|---|
| ECMAScript core (via boa) | M | Web Workers | L |
| Basic DOM APIs | M | WebAssembly | A |
| Events | M | Service Workers | X |
| `fetch` (basic) | R | Shared memory/Atomics | X |
| Promises/async-await | R | JIT compilation | X (only if profiling demands) |

### Storage
| Feature | Class | Feature | Class |
|---|---|---|---|
| Cookies | M | IndexedDB | L |
| LocalStorage | R | Cache Storage | L |
| SessionStorage | R | Quota management | L |

### Media
| Feature | Class | Feature | Class |
|---|---|---|---|
| Images | M | Web Audio | A |
| Audio/Video elements (basic) | L | Media Source Extensions | X |
| Canvas element + 2D context | L | WebGL/WebGPU in canvas | X |

### Security
| Feature | Class | Feature | Class |
|---|---|---|---|
| HTTPS/TLS | M | Sandboxing (reduced) | L |
| Same-Origin Policy | M | Site isolation | X |
| CORS | M | Full sandbox parity w/ Chromium | X (likely never) |
| CSP | L | Secure storage (DPAPI) | R (production) |

### Developer Tools
| Feature | Class | Feature | Class |
|---|---|---|---|
| Console (basic) | L | Sources/debugger | A |
| Elements inspector | L | Performance profiler | A |
| Network panel | L | Storage inspector | L |

---

## 27. Testing Architecture

```text
                     ┌───────────────────────────┐
                     │  Web Platform Compat Tests  │  (subset of WPT, run last, tracks real-world gaps)
                     └───────────────┬───────────┘
                     ┌───────────────▼───────────┐
                     │   Screenshot / Regression   │  (render fixed HTML fixtures, diff against golden PNGs)
                     └───────────────┬───────────┘
                     ┌───────────────▼───────────┐
                     │      Integration Tests      │  (spin up renderer+networking against a local test server)
                     └───────────────┬───────────┘
                     ┌───────────────▼───────────┐
                     │   Component / Crate Tests   │  (html/css/layout/js crates tested in isolation, no IO)
                     └───────────────┬───────────┘
                     ┌───────────────▼───────────┐
                     │         Unit Tests          │  (per-function, fast, run on every save)
                     └────────────────────────────┘
```

- **Unit tests**: standard `#[test]`, colocated per crate; layout math, CSS cascade ordering, URL parsing edge cases.
- **Component tests**: `html` crate parses fixture HTML → asserts DOM shape; `css` crate parses fixture CSS → asserts cascade output; `layout` crate takes a style tree fixture → asserts fragment geometry, entirely without networking/GPU.
- **Integration tests**: real end-to-end (fetch → parse → layout → paint) against a local `wiremock`/hand-rolled HTTP test server, no real network dependency (deterministic, CI-friendly).
- **HTML/CSS/JS/DOM tests**: largely covered by component + integration layers above; JS engine correctness leans on `boa`'s own test-262 conformance results rather than re-deriving them.
- **Networking tests**: protocol-level tests against local servers (redirect chains, cache validation, cookie matching, CORS preflight behavior).
- **IPC tests**: round-trip serialization tests (Phase 2) — every message type gets a serialize/deserialize/equality test, and a fuzz target for the deserializer (untrusted-input boundary).
- **Security tests**: SOP/CORS bypass attempts as explicit negative tests (e.g., "cross-origin fetch without CORS headers must fail"), dangerous-URL-scheme allowlist tests, `file://` isolation tests.
- **GPU tests**: headless `wgpu` device tests where CI hardware allows (GitHub Actions Windows runners have limited GPU support — plan for a software `wgpu` backend or skip-with-warning in CI, run real GPU tests locally/pre-release).
- **Screenshot tests**: golden-image diffing for layout/paint correctness on a curated fixture corpus (grows every time a real bug is found — regression tests are written *from* bug reports, not speculatively).
- **Regression tests**: every fixed bug gets a fixture that reproduces it, permanently, in the relevant tier.
- **Fuzz testing**: `cargo-fuzz` targets on the highest-risk untrusted-input boundaries first — HTML/CSS parsers, IPC deserializers, image decoders (even though decoders are reused crates, the integration glue around them is still worth fuzzing), URL parser.
- **Stress testing**: many-tab memory/CPU behavior (directly tests the tab-tiering system's actual value), long-running-session leak detection.
- **Performance benchmarks**: `criterion` benches for layout (varying DOM size/depth), paint (varying display-list size), parse (varying document size) — tracked over time, regressions flagged in CI.
- **Crash recovery tests**: kill a renderer process (Phase 2) mid-load, assert the tab shows an error state and other tabs are unaffected.
- **Web-platform compatibility tests**: a hand-picked, growing subset of WPT relevant to implemented features — not an attempt at full WPT pass rate, which is a Chromium/Firefox/WebKit-scale undertaking.

---

## 28. Performance Architecture

| Metric | Target (goal, not guarantee) | Primary lever |
|---|---|---|
| Cold startup | < 500 ms to first window paint | Lazy-init non-critical subsystems (devtools, extensions-that-don't-exist-yet, etc.) |
| Memory (idle, 1 tab) | ~200–250 MB, revised from an earlier 150 MB target | `wgpu`'s shader-compilation cache, Boa's GC heap, SQLite, and `html5ever`'s parse stack all add up; Rust buys memory *safety*, not automatically low footprint. Don't market the project on hitting an aggressive number — measure honestly and let the tab-tiering system (below) carry the "efficient with many tabs" story, which is the real differentiator |
| Memory (background tabs) | Materially lower than active tabs | The tab-tiering system (Hot/Warm/Cold/Frozen) is the direct lever here — see §9 tab lifecycle |
| Page load (simple site) | < 1s to first contentful paint on broadband | Streaming HTML parse (§19), incremental layout, avoid blocking on full-document parse before first paint |
| Input latency | < 50 ms perceived | Compositor thread independence from renderer thread (§7/§17) is what makes this achievable even when JS/layout is busy |
| Scroll latency | Matches display refresh, no jank | Scroll offset owned by compositor, not re-triggering layout (§15) |
| Tab switch | Near-instant for Hot/Warm tabs; brief re-hydration acceptable for Frozen tabs | Explicit contract of the tiering system — Frozen tabs *should* take slightly longer to resume; that's the trade being made for memory savings |
| Background tab CPU | ~0 when Frozen | Timers/rAF/JS execution suspended for Frozen tabs (with correctness caveats documented — e.g., `setInterval` drift on resume) |

These are **engineering targets to design around, not SLAs** — stated explicitly per the brief's requirement to distinguish goals from guarantees. Real numbers depend on hardware, target site complexity, and how much of the "Advanced" feature set has landed (more CSS/JS coverage generally costs some performance headroom).

---

## 29. Risk Register

| Risk | Complexity | Impact | Probability | Mitigation |
|---|---|---|---|---|
| JavaScript engine limitations (`boa` compat/perf) | High | High | **High** (revised up — see amendment below) | Start with `boa`; run a real-site JS-execution spike **before layout work begins** (not at M11), against the actual target-site corpus (docs/blogs), so a Boa-insufficiency finding changes course in weeks, not after M11–M13's DOM-binding work is already coupled to Boa's host-object API |
| GPUI/`gpui-ce` upstream volatility | Medium | **High** (project-survival, not just UI polish) | Medium-High | `SoulBackend` trait boundary (§9 amendment) — isolates the blast radius of an upstream break to one backend crate instead of the whole UI layer |
| CSS compatibility (long tail) | Very High | Medium (bounded by Non-Goals) | High (it *will* be incomplete) | Explicit MVP/Phase/Advanced tiers (§14), never claim more coverage than tested |
| Web-platform compatibility broadly | Very High | Medium | High | Curated WPT subset (§27), explicit non-goal of full compatibility |
| GPU rendering complexity (wgpu/DXGI interop with GPUI) | Medium-High | High (blocks everything downstream) | Medium | Resolve the GPUI-backend ADR (§18, ADR-1) *before* M1; prototype the Surface-texture handoff as a spike before committing |
| Multi-process architecture | High | Medium (deferred) | Low (because deferred) | Explicit phase gate at M14-16 (§6); crate APIs already message-shaped to reduce eventual cost |
| Sandbox implementation | High | Medium | Medium | Explicit "reduced, not Chromium-parity" scope (§22/§23) stated up front, avoids false security claims |
| Security (general web-facing attack surface) | High | High | Medium | SOP/CORS/HTTPS as MVP requirements (§23), fuzzing on parser boundaries (§27), explicit site-isolation gap documented |
| HTTP/3 | Medium | Low (HTTP/1.1+2 cover MVP) | Low | Deferred to Phase 2, reused crates (`quinn`/`h3`) |
| Media playback | Medium-High | Low (deferred) | Medium | Media Foundation reuse leverages prior team experience (Aura project); MSE explicitly Advanced |
| Font rendering / text shaping | Medium | High (visible immediately) | Low-Medium | `cosmic-text` reuse (§15) avoids the highest-complexity part (shaping/bidi) |
| IndexedDB | Medium | Low (deferred) | Low | Phase 3, SQLite-backed design sketched early (§20) to avoid painting into a corner |
| Service Workers | Very High | Low (deferred, explicit Non-Goal-adjacent) | Low | Extremely Difficult tier (§18/§26), no MVP dependency on it |
| WebAssembly | Medium | Low (deferred) | Low | `wasmtime` reuse when it lands |
| Developer tools | Medium | Medium (affects the team's own debugging speed) | Medium | A minimal internal `about:` diagnostics/console surface ships *early* (§2 Design Goals), full DevTools UI is Phase 2+ |
| Memory management (tab tiering correctness) | High | High (this is a stated differentiator) | Medium | Dedicated milestone(s), stress tests (§27) specifically targeting many-tab scenarios |
| Performance broadly | Medium-High | High (perceived quality) | Medium | Compositor-thread independence (§7) and streaming parse (§19) as load-bearing early decisions, not late optimizations |
| Solo-developer time/burnout | N/A (project-management risk, not technical) | Very High | High | The entire phased structure of this document exists to keep every milestone independently shippable/usable, so partial progress is never wasted |

---

## 30. Architecture Decision Records

**ADR-1: GPUI rendering backend on Windows**
- *Decision:* Evaluate mainline GPUI (native D3D11 backend) against the `gpui-ce`/`gpui-wgpu` fork (wgpu-unified backend) in a short spike before M1; prefer the wgpu fork if it's stable enough, because it lets the compositor and the browser UI share one GPU device and avoids DXGI shared-texture interop. **Amendment: regardless of which backend wins, it sits behind the `SoulBackend` trait (§9 amendment) — this ADR decides an implementation detail inside `soul-backend-gpui`, not a dependency the rest of the codebase is allowed to see.**
- *Alternatives:* Mainline GPUI + DXGI shared-texture interop with our own `wgpu` compositor.
- *Advantages:* Single device model, simpler interop, potential future cross-platform reuse.
- *Disadvantages:* Fork tracks upstream GPUI independently — risk of drift/lag on GPUI feature updates. **This risk is exactly why the trait boundary exists — it doesn't eliminate GPUI volatility risk, it contains it.**
- *Performance impact:* Likely neutral-to-positive (avoids a texture-sharing round trip).
- *Security impact:* None significant either way.
- *Complexity:* Fork dependency management vs. shared-texture synchronization code — comparable effort, different kind.
- *Long-term consequences:* Committing early avoids a mid-project compositor rewrite; the spike (not guesswork) should settle it, and the trait boundary means a wrong call here is a backend-crate swap later, not a project-wide rewrite.

**ADR-2: Browser process model — deferred multi-process**
- *Decision:* Single process through M13; split GPU (M14), network (M15), renderer-per-window (M16); explicitly defer site isolation.
- *Alternatives:* Multi-process from day one (Chromium-style).
- *Advantages:* Unblocks rendering-engine progress immediately; crate APIs already message-shaped avoid later rewrite cost.
- *Disadvantages:* No crash isolation or sandboxing until M14+; a renderer bug can take down the whole browser in early phases.
- *Performance impact:* Positive early (no IPC overhead), neutral later once split (design accounts for it).
- *Security impact:* Reduced in early phases — accepted, explicitly documented risk (§23), not hidden.
- *Complexity:* Substantially lower up front; deferred complexity is paid later, once the payoff (a working engine) already exists.
- *Long-term consequences:* This is the single decision that makes the rest of the plan achievable by a small team.

**ADR-3: HTML/CSS parsing — reuse `html5ever`/`cssparser`/`selectors`**
- *Decision:* Reuse Servo's low-level parsing/matching crates; build cascade/layout ourselves.
- *Alternatives:* Full from-scratch parser; full reuse of Servo's `style`/Stylo crate (tied to their layout assumptions).
- *Advantages:* Spec-correct tokenization for free; avoids inheriting Stylo's architecture constraints.
- *Disadvantages:* `TreeSink`/`Element` trait integration work at the boundary.
- *Performance impact:* Neutral-to-positive (these crates are optimized, used in production by Servo/Firefox-adjacent tooling).
- *Security impact:* Positive — mature, fuzzed parsers for untrusted input.
- *Complexity:* Lower than from-scratch; integration glue is bounded and well-understood.
- *Long-term consequences:* Frees engineering time for the layout engine, the actual differentiator.

**ADR-4: JavaScript strategy — `boa` first, V8 as a scoped fallback**
- *Decision:* See §18.
- *Alternatives:* V8/`rusty_v8` from the start; QuickJS bindings.
- *Advantages:* Memory safety, build simplicity, "prefer Rust-native" alignment.
- *Disadvantages:* No JIT; compatibility ceiling below V8/SpiderMonkey/JSC.
- *Performance impact:* Acceptable for MVP/Phase 2 target sites; unknown ceiling for JS-heavy apps.
- *Security impact:* Positive (no C++ engine FFI surface) unless/until a V8 fallback is adopted.
- *Complexity:* Lower initially; a later engine swap would be a major, explicitly-budgeted milestone, not a drop-in change.
- *Long-term consequences:* This is the decision most likely to need revisiting — flagged as such rather than presented as final.

**ADR-5: IPC — message-shaped from day one, transport swapped later**
- *Decision:* See §8.
- *Alternatives:* Design purely in-process APIs first, retrofit IPC-shaped boundaries later.
- *Advantages:* No architectural rewrite at M14+; the process split becomes a transport change, not a redesign.
- *Disadvantages:* Slightly more ceremony (enums instead of trait method calls) in early phases, for a payoff that only matters later.
- *Performance impact:* Negligible overhead in-process (enum dispatch, no serialization until Phase 2).
- *Security impact:* Positive once IPC is real (validation is already the natural place to add it).
- *Complexity:* Modest upfront tax.
- *Long-term consequences:* Directly enables ADR-2's "no rewrite" claim.

**ADR-6: GPU API — `wgpu` over raw Vulkan/D3D12**
- *Decision:* See §21.
- *Alternatives:* Raw Vulkan; raw D3D12.
- *Advantages:* Memory safety, cross-backend flexibility, less `unsafe`, ecosystem alignment with GPUI's own direction.
- *Disadvantages:* Slightly less low-level control than raw APIs; abstraction overhead (generally small for this workload).
- *Performance impact:* Acceptable for a 2D-compositing-dominated workload; a game engine's workload would weigh this differently.
- *Security impact:* Positive (less hand-rolled unsafe GPU synchronization code).
- *Complexity:* Lower than raw APIs.
- *Long-term consequences:* Keeps the option of non-Windows backends open without extra work, if the project ever expands platforms.

**ADR-7: Storage — SQLite over a bespoke database**
- *Decision:* See §20.
- *Alternatives:* Custom binary formats per store; an embedded KV store (`sled`, `redb`).
- *Advantages:* ACID, mature tooling, matches the proven Chromium/Firefox approach, `FTS5` for history search "for free".
- *Disadvantages:* Slightly heavier than a minimal KV store for the simplest cases (e.g., SessionStorage, which is kept in-memory instead precisely to sidestep this).
- *Performance impact:* Fine for browser-scale data volumes; WAL mode handles concurrent read/write well.
- *Security impact:* Neutral (encryption-at-rest is a separate, later decision — §20/§23).
- *Complexity:* Low (well-documented `rusqlite` usage).
- *Long-term consequences:* Easy to reason about, easy to migrate schemas.

*(ADRs 8–15 — Async runtime, Networking stack, Windows abstraction, Sandbox strategy, Site isolation strategy, and Dependency philosophy — are each already resolved inline in §7, §19, §22, §23, §6/§23, and §25 respectively, and are summarized rather than repeated here to avoid duplicating the same decision twice; each carries the same alternatives/advantages/disadvantages structure and can be split into standalone ADR documents under `docs/adr/` as the project matures.)*

**ADR-16: Milestone renumbering M20/M21 (implementation-tracking)**
- *Decision:* The repository's actual commit sequence renumbers two milestones relative to this plan's original list: **M20 = Downloads** (download manager + MOTW) and **M21 = Compositor & performance-benchmark harness**. The original **M20 Compatibility (WPT subset)** no longer holds a milestone number and is tracked as **not started** (see §31). M22 (Security) and M23 (Production) match the plan's intent. §31's status table is the authoritative source for actual milestone state; AGENTS.md §0 milestone verification should read from it.
- *Rationale:* Downloads and compositor/benchmark work landed before any WPT-subset tooling existed; the renumbering keeps the plan aligned with the repository rather than carrying a stale numbering.
- *Consequence:* No architecture changed — M10b (GPU compositor), M14–M16 (process splits), and M18 (Media Foundation) remain outstanding as originally scoped; only milestone numbering and explicit status tracking changed.

---

## 31. Development Milestones

Each milestone lists: **Objective · Components · Depends on · Tasks · Tests · Definition of Done · Risks · Explicitly NOT in scope yet.**

**Status key (as of 2026-08-17, verified against repository code):**
- **Complete** — the milestone's components exist, are wired, and are covered by passing tests.
- **Partial** — real implementation exists for part of the milestone; named items are missing.
- **Simulated** — code exists behind the milestone's API surface, but the OS/GPU/process integration the milestone exists to prove is not real (headless backend, in-memory transport, state-machine stand-in).
- **Not started** — no implementation.

| Milestone | Plan intent | Repo status |
|---|---|---|
| Spike 0 | GPUI + Boa de-risking (ADR-1, ADR-4) | **Complete** — both resolved; `docs/spike-0-results.md` (6/6 corpus tests pass) |
| M0 Foundation | 25-crate workspace, tracing, CI | **Complete** — 25 crates + benchmarks, Edition 2024, Rust 1.97.1, CI green |
| M1 GPUI shell | real window via `SoulBackend` | **Complete** — real native window via GPUI (git zed gpui 0.2.2 + gpui-component, confined to `soul-backend-gpui`); engine frames presented via `ImageSource::Render`; window existence proven by Win32 `EnumWindows` smoke test |
| M2 Window + input | OS input events, DPI | **Complete** — raw GPUI mouse/keyboard/wheel events route through `InputRouter`; GPUI bounds and scale factor emit `SoulEvent::WindowResized`; dynamic window resize recreates viewport buffers; link hit-testing emits navigation events; page scrolling updates retained viewport rasters and document-space hit-testing; shared tab-strip chrome renders and routes create/select/close events through core-owned `TabManager` state, with per-tab navigation controllers, titles/loading state, and retained frame restoration; new tabs render a static local page through the engine pipeline; Win32 input smoke test passes |
| M3 URL + navigation | nav state machine, stub fetch | **Complete** — `NavigationController` state machine, `NavigationId` race handling, wired into the soul-shell live-navigation path |
| M4 Networking | HTTP(S), DNS, redirects, cookies | **Partial** — real HTTP/1.1 + TLS 1.2/1.3 (hyper 1, rustls, webpki-roots) wired into shell live navigation; RFC 9110 HTTP payload decompression (`gzip`, `deflate` via `flate2`) with `Accept-Encoding` negotiation; bounded redirect following (5 hops, POST→GET on 301/302/303); `Set-Cookie` multi-header extraction and RFC 6265bis parsing into `storage::Cookie`; no hickory DNS, no HTTP/2 |
| M5 HTML parser | html5ever → DOM | **Complete** — plus `parse_html_with_styles` extracting author `<style>` sheets |
| M6 DOM API | NodeId arena, mutation, query | **Complete** |
| M7 CSS + style | cascade, computed style | **Complete** — full W3C `box-sizing` (`content-box`/`border-box`), 1-4 value margin/padding/border shorthands, `border-radius`, HSL/HSLA and 140+ named colors, `line-height`, `font-style`, `text-decoration`, inline `<style>` and external `<link>` cascade resolution |
| M8 Layout | block/inline/flex + text shaping | **Complete** — block + inline layout, CSS 2.1 §8.3.1 vertical margin collapsing between in-flow siblings, W3C box-sizing geometry, cosmic-text shaping; CSS Flexbox Level 1 (`Display::Flex`, `FlexDirection`, `FlexWrap`, `JustifyContent`, `AlignItems`, `AlignSelf`, `flex-grow`/`shrink`/`basis`) via `taffy` 0.13 integration in `layout::flex` |
| M9 Paint | display list | **Complete** |
| M9.5 A11y skeleton | semantic data in fragment tree | **Partial** — `A11yNode` tree (roles, aria-label, bounds) real + tested; no UIA provider, not wired to `platform-windows` |
| M10a Software raster | CPU pixels on screen | **Partial** — `tiny-skia` rasterizer real; frames reach disk as PNG via `soul-shell --output` (verified against live sites incl. image subresources); presented on native GPUI window; still CPU blit rather than GPU surface swap |
| M10b GPU compositor | wgpu compositor | **Partial** — wgpu 24 `GpuContext`/`GpuTexture` real but headless (no surface/DXGI/swapchain); `GpuCompositor` composites on CPU then uploads the whole frame; no GPU render pass, no damage-rect upload, not wired into the shell |
| M11 Basic JS | boa + event loop + DOM bindings | **Complete** — boa 0.21, task/microtask queues, console, full DOM manipulation bindings (`document.getElementById`, `querySelector`, `createElement`, `appendChild`, `setAttribute`, `getAttribute`, `removeAttribute`, `classList.add/remove/toggle/contains`), wired into inline `<script>` and external `<script src="...">` execution pipeline in document order |
| M12 Web APIs | fetch, timers, Promises | **Complete** — timers + Promise/microtask ordering real; `window.fetch()` Promise binding wired to asynchronous networking via `HttpClient` in `web-api`; `window.location` & `window.navigator` registered and tested end-to-end |
| M13 Storage | SQLite persistence | **Complete** (MVP scope) — SQLite WAL (cookies/history/bookmarks/LocalStorage) + `localStorage` and `sessionStorage` JS bindings registered and wired to script execution in `soul-shell`; `Cookie::parse` for RFC 6265bis header values; RFC 9111 `HttpCacheStore` (`gzip`/`deflate` decompressed bodies persisted, `max-age` freshness, `ETag`→304 metadata refresh, `no-store`/`private` enforcement) |
| M14 GPU split + IPC | GPU process, real IPC | **Partial** — ipc enums/framing/dispatcher real; tokio Windows named-pipe transports real + tested roundtrip; still single process — nothing uses pipes outside tests, no GPU process |
| M15 Network split | network process | **Simulated** — `NetworkService` runs as an in-process tokio task |
| M16 Sandboxing | renderer process, Job Objects | **Partial** — `JobObject`, `RestrictedToken`, `ProcessLauncher` real; launcher job-lifetime bug fixed via `SandboxedChild` (job kept alive, `kill_job` tested); restricted token still never applied, nothing spawned outside tests |
| M17 Advanced Web APIs | Workers, IndexedDB, richer fetch | **Partial** — `WebWorker` thread + SQLite `IndexedDbStore` real; JS IndexedDB bindings and richer fetch absent |
| M18 Media | MF playback + Canvas 2D | **Partial** — `ImageDecoder` (PNG/JPEG/WebP/GIF/SVG), `Canvas2DContext` real; `MediaPipeline` = state machine + solid-color `generate_frame`, no Media Foundation decode |
| M19 DevTools | inspector/console/network | **Complete** — `CdpServer` JSON-RPC, DOM/Network/Console monitors |
| M20 Downloads *(renumbered)* | download manager + MOTW | **Complete** — `DownloadManager` + `Zone.Identifier` MOTW |
| M21 Compositor + perf *(renumbered)* | damage/layers, benchmark harness | **Partial** — CPU `DamageTracker`/layer blits + benchmark harness real; GPU path (M10b) outstanding |
| M22 Security | CSP, DPAPI, private browsing | **Partial** — `CspPolicy` directive parse, `Dpapi`, `PrivateBrowsing` real; CORS + mixed-content enforced on all live subresources (`<img>`, `<link rel="stylesheet">`, `<script src="...">` fetches go through `fetch_with_security_context` against document origin) |
| M23 Production | signed installer, updates, crash reporting | **Partial** — shell wires the engine crates and navigates live URLs end-to-end (fetch → parse → inline & external scripts + Web Storage + fetch() → external `<link>` & inline CSS cascade → layout → paint → raster → PNG + a11y tree, incl. `<img>` subresources) presented in a real GPUI window with multi-tab support; all files modularized under 280 lines; workspace test suite green (120+ tests passed) |

**Note (current wiring work):** `soul-shell` has a fully connected path — `NavigationController` drives live HTTP(S) fetches through the full rendering pipeline, with per-stage timings, PNG screenshot output, inline and external `<script src="...">` execution with Web Storage and `fetch()` DOM mutations, external `<link rel="stylesheet">` CSS parsing and cascade resolution, accessibility-tree extraction (verified against live sites and fixtures), and presentation in a genuine native GPUI window with tab switching, URL input, retained scrolling, and dynamic resizing. Integration test suites are organized by topic in `crates/soul-shell/tests/navigation_pipeline_tests.rs`, `tests/script_storage_tests.rs`, and `tests/viewport_scroll_resize_tests.rs`. Still unwired: named pipes across OS processes, `ProcessLauncher` child spawns.

**Milestone renumbering:** the repository's commit sequence renumbered two milestones relative to the original list — former M20 (Compatibility / WPT subset) is **not started** and no longer holds a number; Downloads was slotted into M20, and M21 became Compositor + performance-benchmark harness instead of the original performance-optimization milestone. M22/M23 match the plan's intent. Plan tracking (AGENTS.md §0) should treat the status table above as authoritative.

**Spike 0 — De-risking spikes (run before M1, in parallel with each other, not sequentially)**
Objective: answer the two highest-uncertainty questions in the plan *before* committing architecture around either answer. Components: throwaway/prototype code, not production crates. Depends on: nothing. Tasks: **(a)** GPUI Surface spike — a minimal window that embeds an externally-rendered `wgpu` (or D3D11-shared) texture via GPUI's `Surface` element, resolving ADR-1; **(b)** Boa corpus spike — run `boa` headless against the actual intended target-site JS (docs/blog-style sites), cataloguing any parse/execution failures against real-world code, not synthetic test-262 cases. Tests: n/a (spike code is disposable). DoD: ADR-1 is resolved with evidence; Boa's viability against the real target corpus is either confirmed or a V8 pivot decision is made **now**, before any DOM-binding code exists. Risks: skipping this step and discovering either answer late is the single most expensive mistake available in this plan. NOT yet: any production code depending on either answer.

**M0 — Project Foundation**
Objective: workspace exists, builds, CI runs. Components: `common`, workspace `Cargo.toml`, `rust-toolchain.toml`. Depends on: nothing. Tasks: crate skeletons per §24, `tracing` setup, CI (build + test on Windows runner). Tests: `cargo test` passes on an empty workspace. DoD: green CI on a trivial commit. Risks: none significant. NOT yet: any actual feature code.

**M1 — GPUI Browser Shell**
**Status: Complete.** A real native window opens via GPUI (git `zed-industries/zed` gpui 0.2.2 + `gpui-component`, both confined to this crate), presenting the latest engine frame full-window. Window existence verified by a Win32 `EnumWindows` smoke test (`crates/soul-shell/tests/window_smoke_tests.rs`, `--ignored`). Window-close events are forwarded to the `SoulBackend` handler. Outstanding: input-event routing into `InputRouter` (M2) and browser-UI widgets (tab strip/omnibox built from scratch on gpui elements).
Objective: an empty window opens and closes cleanly, through the `SoulBackend` trait rather than direct GPUI calls. Components: `soul-shell`, `soul-ui` (trait), `soul-backend-gpui` (impl). Depends on: M0, Spike 0(a) resolved. Tasks: define `SoulBackend`, window creation, basic event loop wiring, app icon/title — all `gpui::*` usage confined to `soul-backend-gpui`. Tests: manual + a smoke test that the binary launches and exits 0 headless where possible. DoD: `soul.exe` opens a native window, and `soul-core` has zero `gpui` in its dependency tree. Risks: getting the trait boundary wrong here is expensive to fix later — worth extra review time. NOT yet: tabs, any page content.

**M2 — Window + Input System**
**Status: Partial.** Raw GPUI elements now provide Soul toolbar buttons, an from-scratch omnibox, and a shared tab-strip view; mouse, keyboard, and wheel events route through `InputRouter` and emit `SoulEvent::InputRouted`; GPUI bounds and scale factor emit `SoulEvent::WindowResized`; page anchor hit-testing emits `SoulEvent::LinkActivated`; wheel input below the toolbar produces bounded page-scroll commands, retained document rasters update without refetching, and hit-testing converts viewport coordinates back into document coordinates. Tab create/select/close events now route through the shell driver into the core-owned `TabManager`; each tab retains its own navigation controller, rendered result, title, loading state, and scroll state, and selecting a tab restores its retained frame. New tabs render a static local page through the same HTML → CSS → layout → paint → raster pipeline as normal content. Keyboard editing, Enter navigation, live frame updates, and Win32 input injection are verified. Outstanding: desktop scroll smoke verification.
Objective: input events reach application code correctly. Components: `soul-ui`, `platform-windows`. Depends on: M1. Tasks: mouse/keyboard/wheel routing, DPI-aware sizing, multi-monitor window placement. Tests: input-routing unit tests with synthetic events. DoD: clicking/typing in the (still empty) window is observable in app state. Risks: DPI edge cases (mitigated by prior Aura experience). NOT yet: hit-testing into page content (doesn't exist).

**M3 — URL + Navigation (skeleton)**
Objective: omnibox accepts a URL and the navigation state machine (§11) runs end-to-end with a stubbed fetch. Components: `soul-core`, `soul-ui`. Depends on: M2. Tasks: `url` crate integration, navigation state machine, stub network response. Tests: navigation-race unit tests (stale `NavigationId` discarded). DoD: typing a URL transitions through the full state machine to a stub "loaded" state. Risks: low. NOT yet: real networking.

**M4 — Networking**
**Status: Partial.** Real HTTP/1.1 + TLS 1.2/1.3 client (hyper 1, rustls, webpki-roots). Missing vs. plan: hickory-resolver DNS (OS resolver used), redirect following, cookie jar (cookies live in `storage`, unwired), HTTP/2.
Objective: real HTTP(S) GET requests complete. Components: `networking`. Depends on: M3. Tasks: `hyper`+`rustls`+`hickory-resolver` wiring, redirect handling, basic cookie jar. Tests: integration tests against a local test server (§27); TLS cert validation tests. DoD: fetching a real HTTPS URL returns bytes into `soul-core`. Risks: TLS trust-store platform quirks on Windows. NOT yet: HTTP/2/3, caching, CORS.

**M5 — HTML Parser**
Objective: HTML bytes become a DOM. Components: `html`, `dom`. Depends on: M4 (bytes to parse), can develop in parallel with fixtures. Tasks: `html5ever` `TreeSink` impl against `dom`, encoding sniffing. Tests: fixture-based DOM-shape assertions (§27). DoD: a real webpage's HTML produces a correct DOM tree (verified against fixtures, not visually yet). Risks: low (mature reused parser). NOT yet: `<script>` execution, CSS.

**M6 — DOM (API surface)**
Objective: DOM supports the mutation/query API JS and layout will need. Components: `dom`. Depends on: M5. Tasks: `NodeId` arena, mutation API + dirty-bit invalidation hooks, `querySelector` via `selectors`. Tests: mutation/invalidation unit tests. DoD: programmatic DOM mutation works and correctly marks invalidation. Risks: low. NOT yet: MutationObserver JS binding (needs JS engine first).

**M7 — CSS Parser + Style System**
Objective: CSS becomes computed styles on DOM nodes. Components: `css`. Depends on: M6. Tasks: `cssparser` rule parsing, `selectors` matching against `dom::Element`, cascade (§14), computed-value resolution for the MVP property set. Tests: cascade-ordering unit tests, fixture-based computed-style assertions. DoD: a DOM + stylesheet produces correct computed styles for the MVP property set. Risks: property-set scope creep (mitigate by strict adherence to §14's MVP list). NOT yet: Flexbox/Grid, animations.

**M8 — Layout**
Objective: computed styles become positioned boxes. Components: `layout`, `text-shaping`. Depends on: M7. Tasks: block/inline layout (§15), `cosmic-text` integration for line breaking/shaping, absolute/fixed positioning pass. Tests: fragment-geometry fixture tests, `criterion` layout benchmarks (baseline). DoD: a real page's DOM+CSS produces correct box geometry (verified numerically, not yet visually). Risks: this is the highest-complexity from-scratch subsystem — budget the most time here. NOT yet: Flexbox/Grid (taffy integration), scrolling.

**M9 — Paint**
Objective: fragment tree becomes a display list. Components: `paint`. Depends on: M8. Tasks: display-list builder, stacking-context tree, basic clip/text/rect/image draw commands. Tests: display-list-shape fixture tests. DoD: a display list correctly represents a laid-out page. Risks: low. NOT yet: GPU rasterization/compositing (next milestones).

**M9.5 — Accessibility Skeleton (amendment)**
**Status: Not started.** No role/semantic/ARIA data exists in `layout`/`paint`.
Objective: carry minimal semantic information (name/role/bounds) alongside the fragment/display-list data from the moment it exists, without yet exposing it to any screen reader. Components: `layout`, `paint`. Depends on: M9. Tasks: attach a lightweight semantic-role field to layout boxes (derived from element type/ARIA attributes already present in the DOM), threaded through to the display list. Tests: fixture tests asserting the semantic tree's shape matches the fragment tree. DoD: the data exists and is queryable internally — no UIA provider yet. Risks: low if done now; this exists specifically because it is *not* low-risk to retrofit later (per review feedback — a real UIA provider is still a later, larger milestone, not pulled forward here, but the data it will need is captured from day one). NOT yet: an actual UI Automation provider, screen-reader testing, keyboard-navigation semantics beyond what already falls out of focus handling.

**M10a — Software Raster to Screen**
**Status: Simulated.** CPU rasterizer real and tested; the resulting frame is stored in the in-memory backend's window state, never presented on a screen (no real window exists).
Objective: prove the display-list-to-pixels pipeline is correct, isolated from any GPU interop risk. Components: `raster` (tiny-skia). Depends on: M9.5. Tasks: CPU raster of display lists to a bitmap, presented via the simplest possible path the `SoulBackend` trait supports (e.g., an image/software-surface element) rather than a GPU texture handoff. Tests: screenshot/golden-image tests begin here (§27) — and are easier to get right without GPU nondeterminism in the loop. DoD: **a real webpage renders visually on screen inside a tab**, via the CPU path. This is the project's first genuinely demo-able milestone, and it arrives without having to simultaneously debug GPU synchronization. Risks: low — this is precisely the point of splitting it out. NOT yet: GPU compositing, damage tracking, acceptable performance at scale (CPU raster is a correctness checkpoint, not the shipping path).

**M10b — GPU Compositor**
**Status: Not started.** No `wgpu` dependency in the workspace; `gpu` crate is an empty shell; `compositor` does CPU-only layer composition + damage tracking.
Objective: replace the M10a software path with the real `wgpu` compositor, now that display-list correctness is already proven. Components: `compositor` (wgpu), `gpu`, `soul-backend-gpui`. Depends on: M10a, Spike 0(a) resolved. Tasks: texture upload, quad compositing, GPU-texture handoff through `SoulBackend`'s surface-embedding capability (§9). Tests: same screenshot suite as M10a, now diffed to confirm GPU output matches the CPU-raster baseline (a strong correctness check specific to this split). DoD: the shipping GPU-accelerated path renders correctly and matches the M10a baseline. Risks: driver bugs, DXGI keyed-mutex issues, device-lost recovery, mixed-DPI multi-monitor cases — real risk, but now isolated from display-list-correctness risk, which is the whole point of the M10a/M10b split. NOT yet: damage-tracking optimization, GPU-side rasterization (Phase 3).

**M11 — Basic JavaScript**
Objective: `<script>` executes and can read/write a minimal DOM API. Components: `javascript`, `web-api`. Depends on: M10b (want visual feedback for debugging JS-driven DOM changes), Spike 0(b) already resolved. Tasks: `boa` embedding, hand-written event loop (§18), `console.log`, `querySelector`/`addEventListener`/`classList`/basic `innerHTML`. Tests: script fixtures asserting DOM mutation results. DoD: a page with a simple script (e.g., toggling a class on click) works end-to-end. Risks: low relative to the original plan — the Boa-viability question was already answered at Spike 0, before any binding code was written, rather than discovered here. NOT yet: Promises/async, `fetch` from JS.

**M12 — Web APIs**
**Status: Complete.** Timers, Promise/microtask ordering, and `window.fetch()` Promise bindings are implemented and wired to asynchronous networking via `HttpClient` in `web-api`, tested end-to-end with live and mock server flows.
Objective: `fetch`, timers, and richer DOM coverage. Components: `web-api`, `javascript`. Depends on: M11, M4. Tasks: Promise/microtask integration (§18), `setTimeout`/`setInterval`, `fetch` bound to `networking`. Tests: async-ordering tests (microtask vs. macrotask), `fetch` integration tests. DoD: a page can `fetch()` data and update the DOM asynchronously. Risks: event-loop ordering bugs are subtle — budget real test time. NOT yet: Workers, IndexedDB.

**M13 — Storage**
**Status: Complete (MVP scope).** SQLite WAL storage (cookies/history/bookmarks/LocalStorage) real and tested; `localStorage` (SQLite-backed) and `sessionStorage` (in-memory) JS bindings registered and wired to inline script execution in `soul-shell`.
Objective: cookies/LocalStorage/history/bookmarks persist. Components: `storage`. Depends on: M12 (LocalStorage needs JS bindings), M4 (cookies need networking). Tasks: SQLite schema + migrations, cookie jar persistence, LocalStorage JS bindings, history/bookmarks backing soul-ui. Tests: persistence round-trip tests, migration tests. DoD: closing and reopening the browser preserves cookies/history/bookmarks; **this is effectively MVP-complete** (see §32). Risks: low. NOT yet: IndexedDB, Cache Storage, quotas.

**M14 — GPU Acceleration (process split)**
**Status: Simulated.** `ipc` crate real: typed command/event enums, length-prefixed JSON framing, in-memory + generic stream transports, dispatcher. Single process only — no OS named-pipe boundary, no GPU process.
Objective: GPU work moves to its own process. Components: `gpu`, `ipc`, ADR-5's IPC layer goes from theoretical to real. Depends on: M13 (don't split a process before the single-process version is stable). Tasks: IPC transport implementation (§8), shared-texture handoff across the process boundary, GPU-process crash handling (device-lost recovery). Tests: IPC round-trip/fuzz tests begin here (§27). DoD: killing the GPU process doesn't kill the browser process; a device-lost event recovers gracefully. Risks: the first real multi-process bugs (races, partial-message handling) show up here. NOT yet: renderer/network process splits.

**M15 — Multi-Process Architecture (network split)**
**Status: Simulated.** `NetworkService` processes ipc messages as an in-process tokio task; not a separate OS process.
Objective: networking moves to its own process. Components: `networking`, `ipc`. Depends on: M14. Tasks: extend IPC layer, connection-pool-across-tabs correctness in the new process boundary. Tests: integration tests re-run against the split architecture. DoD: killing the network process is recoverable (in-flight requests fail gracefully, browser process stays up). Risks: subtle behavior changes vs. in-process (timing, buffering). NOT yet: renderer split.

**M16 — Sandboxing (renderer split, coarse)**
**Status: Partial.** Win32 Job Object (memory limits, UI lockdown, kill-on-close) and restricted-token code real; no renderer process spawned into a Job/token yet.
Objective: each window's renderer runs in its own, reduced-privilege process. Components: `sandbox`, `platform-windows`, `ipc`. Depends on: M15. Tasks: `CreateProcess` with restricted tokens, Job Object setup, per-window renderer IPC wiring, crash-isolated tab error UI (§11). Tests: crash-recovery tests (kill a renderer process, assert isolation). DoD: a renderer crash shows an error page for that tab only. Risks: Windows sandboxing APIs are fiddly and under-documented for this exact use case — budget generously. NOT yet: site isolation (per-origin, not per-window), full sandbox parity with Chromium (explicitly never claimed).

**M17 — Advanced Web APIs**
**Status: Partial.** `WebWorker` (real OS thread + mpsc + second `JsRuntime`) and SQLite-backed `IndexedDbStore` real; JS bindings for IndexedDB and richer `fetch` not implemented.
Objective: Workers, IndexedDB, richer `fetch`. Components: `javascript`, `web-api`, `storage`. Depends on: M16. Tasks: per-Worker `boa` VM instances + `postMessage`, IndexedDB SQLite-backed implementation. Tests: Worker message-passing tests, IndexedDB transaction/versioning tests. DoD: a page can use a Worker and IndexedDB for real work. Risks: IndexedDB spec surface is larger than it looks. NOT yet: SharedArrayBuffer/Atomics, Service Workers.

**M18 — Media**
**Status: Simulated.** `image-decode` (image crate + resvg/usvg) real; `Canvas2DContext` (tiny-skia) real; `MediaPipeline` is a playback state machine only — no Media Foundation bindings, no audio/video decode.
Objective: `<audio>`/`<video>` playback, `<canvas>` 2D. Components: `media`, `image-decode`. Depends on: M17. Tasks: Media Foundation playback pipeline (reuses Aura-project MF experience), Canvas 2D context implementation against compositor primitives. Tests: playback smoke tests, canvas-draw fixture tests. DoD: common video formats play; basic canvas drawing works. Risks: codec/container edge cases are endless — scope to common formats only. NOT yet: MSE, WebGL/WebGPU canvas.

**M19 — Developer Tools**
Objective: a real inspector exists. Components: `devtools`. Depends on: M18 (or can start earlier in parallel — the minimal `about:` diagnostics surface from §2 is not this milestone, it's a prerequisite that exists from M1). Tasks: Elements panel (DOM tree + computed style viewer), Console, Network panel, Storage inspector. Tests: manual + snapshot tests of panel data models. DoD: a developer can inspect a real page's DOM/network/console without leaving the browser. Risks: scope creep (a full DevTools is itself a large product). NOT yet: Sources/debugger with breakpoints (Advanced).

**M20 — Downloads *(renumbered from the original plan's M20 Compatibility)* — Status: Complete**
Objective: resumable file downloads with Windows Mark-of-the-Web protection. Components: `downloads`, `networking`, `platform-windows`, `storage`. Depends on: M19. Tasks: `DownloadManager` (tokio HTTP streaming to disk, progress tracking, cancel), MOTW `Zone.Identifier` attachment on completion. Tests: download-lifecycle and MOTW tests. DoD: downloading a URL writes the file and tags it with `Zone.Identifier`. Risks: low (reuses the M4 HTTP client). NOT yet: download resumption across restarts, quota/safety heuristics.

**M21 — Compositor & Performance Benchmarks *(renumbered)* — Status: Complete**
Objective: compositor layer/damage infrastructure plus a repeatable pipeline benchmark harness. Components: `compositor`, `raster`, `benchmarks`. Depends on: M20. Tasks: damage tracking (dirty-rect accumulation), layer composition with transforms/opacity, full html→raster pipeline timing benchmark. Tests: compositor unit tests, benchmark-harness test. DoD: layers composite with correct damage; the benchmark harness produces per-stage timings. Risks: low. NOT yet: wgpu GPU compositing (M10b still outstanding), profile-driven optimization against the §28 targets on real sites (the original M21 performance intent), WPT compatibility.

**M22 — Security Hardening**
**Status: Partial.** CSP directive parsing (default/script/style/connect/img-src + source matching), DPAPI encryption (`CryptProtectData`), and a `PrivateBrowsing` ephemeral profile are real. Not implemented: mixed-content blocking, CSP nonce/hash sources and violation reporting, remainder of the §23 table.
Objective: close known gaps from §23/§29 as far as is realistic. Components: cross-cutting, `sandbox`. Depends on: M21. Tasks: CSP full coverage, mixed-content blocking, MOTW/download security, DPAPI-encrypted cookie/password storage, private browsing. Tests: negative security tests (§27) expanded. DoD: the Security Architecture table in §23 is fully "Yes" up to its stated Phase-2/Production scope — site isolation remains explicitly out of scope. Risks: security work never truly "finishes" — this milestone closes the *planned* gaps, not all conceivable ones. NOT yet: site isolation.

**M23 — Production Release**
**Status: Partial.** `soul-shell` wires the full engine pipeline and all subsystem crates; workspace builds and tests green. Not implemented: code signing, updater, crash-report pipeline, installer, final QA per §21.
Objective: ship something real. Components: cross-cutting, `soul-shell` (installer/updater). Depends on: M22. Tasks: code-signing, an update mechanism (even a simple "check for new installer" flow), crash reporting pipeline, final QA pass against §21's Production Definition. Tests: full regression suite green, manual release QA checklist. DoD: an installable, signed, auto-checking-for-updates build that meets §21. Risks: "production" scope creep — hold the line at §21's explicit list. NOT yet: anything not in §21.

**Deferred — Web-Platform Compatibility (formerly M20)**
Objective: measured improvement against the curated WPT subset (§27). Components: cross-cutting. Depends on: M21 (current numbering). **Status: Not started** — no WPT-subset harness exists; workspace `tests/` is empty and no screenshot/golden-image tooling is in place. The milestone's intent is retained; it re-enters the numbered sequence when a harness exists.

---

## 32. MVP Definition

The MVP is reached at the end of **M13**. It can:

**Status note (2026-08-14):** the M13-era milestone code exists and most items in this list are implemented and unit/integration-tested. A real native window now opens (M1 complete — GPUI), and `soul-shell` navigates live URLs end-to-end (fetch → parse → style → layout → paint → raster → window + PNG + a11y tree), verified against real HTTPS sites. Remaining gaps vs. this list: no on-screen scrolling, no interactive JS in the shell flow, and M4 networking is not yet wired into CSS/JS subresource fetching. Treat this list as the capability contract; treat §31's status table as the current state.

- Open URLs typed into an omnibox, over HTTP and HTTPS (with real TLS validation).
- Perform DNS resolution and HTTP/1.1 (and HTTP/2) requests, with redirects, gzip/brotli decompression, and a working cookie jar.
- Parse real-world HTML into a correct DOM (via `html5ever`).
- Parse real-world CSS for the MVP property set (§14) and compute styles via a correct cascade.
- Render basic CSS: box model, block/inline flow, basic positioning, colors, fonts, backgrounds, borders.
- Display text (shaped via `cosmic-text`) and images (via the `image` crate).
- Scroll, at 60fps-class smoothness, decoupled from layout.
- Follow links and perform full navigation, with a correct back/forward stack and no navigation races.
- Run multiple tabs in one GPUI-based browser UI, with working omnibox/toolbar/tab strip.
- Execute basic JavaScript: DOM query/mutation, event listeners, `console.log`, `setTimeout` — enough for simple interactivity, not enough for a modern SPA.
- Enforce baseline security: HTTPS validation, Same-Origin Policy, CORS for `fetch`, dangerous-URL-scheme allowlisting.
- Persist cookies, history, and bookmarks across restarts.

**What the MVP explicitly does not support** (by design, not oversight): Flexbox/Grid layout, animations/transitions, `<canvas>`/`<video>`/`<audio>` beyond element parsing, Web Workers, IndexedDB, Service Workers, WebAssembly, multi-process isolation of any kind (still one process), any sandboxing, CSP, private browsing, extensions, full accessibility tree, developer tools beyond ad hoc logging, HTTP/3.

This MVP is a real, usable browser for static-to-mildly-dynamic content — documentation sites, blogs, many marketing/informational sites, simple internal tools — and a legitimate demo of the whole pipeline working end to end. It is not a daily-driver replacement for a mainstream browser, and should not be presented as one.

## 33. Production Definition

Before calling this a "production browser," it needs (beyond the MVP, cumulative through M23):

- **Modern HTML**: Shadow DOM, `<template>`, full table layout, custom elements lifecycle.
- **Modern CSS**: Flexbox, Grid, animations/transitions, container queries, `:has()`, custom properties.
- **JavaScript compatibility**: broad ECMAScript coverage (whatever `boa`'s maturity supports, or a V8 migration per ADR-4 if that path was taken), Promises/async, Workers, WebAssembly.
- **Web APIs**: `fetch` fully, IndexedDB, Cache Storage, `MutationObserver`, History API, reasonably broad DOM coverage.
- **GPU acceleration**: full compositor path (§17/§21), ideally with GPU-side rasterization by this point.
- **Multi-process architecture**: GPU, network, and renderer-per-window processes (§6), all shipped and stable.
- **Sandboxing**: Job Object + restricted token + AppContainer renderer sandboxing (§22/§23), explicitly *not* claimed equivalent to Chromium's kernel-level attack-surface reduction.
- **Site isolation**: still explicitly **not required** for this project's definition of "production" — stated as a known, permanent-for-now limitation rather than a temporary gap, because closing it fully is a different order of project.
- **Storage**: full LocalStorage/IndexedDB/Cache Storage, quotas, DPAPI-encrypted secrets, private browsing.
- **Networking**: HTTP/1.1/2/3, full CSP, mixed-content blocking, proxy support.
- **Media**: common audio/video playback via Media Foundation, Canvas 2D; MSE/WebGL-in-canvas remain out of scope even at "production."
- **Accessibility**: this remains the **largest honest gap** in this plan relative to a real production browser, and it is more than a nice-to-have gap — for any audience beyond personal/hobbyist use, shipping a browser that can't interface with Windows Narrator or other assistive technology is a genuine barrier to use, and in some jurisdictions (EU Accessibility Act, ADA-adjacent requirements in the US) a compliance concern for "production" framing specifically. M9.5 (§31, added on review) now carries minimal semantic data (name/role/bounds) alongside the fragment tree from early on specifically so a real UI Automation provider is an *addition* on top of existing data at production time, not a retrofit onto a layout engine that never captured it. The UIA provider implementation itself is still not scoped as a numbered milestone here — that remains real, unbudgeted work a team needs to plan for explicitly before calling this "production" in any context where accessibility compliance matters.
- **Developer tools**: Elements/Console/Network/Storage panels at minimum (M19); Sources/debugger is a stretch goal.
- **Crash recovery**: per-tab isolation (M16) plus session restore (already MVP-adjacent, §10).
- **Automatic updates**: a real, signed update mechanism (M23) — not optional for anything installed on real users' machines.
- **Security hardening**: the full §23 table, minus the explicitly-deferred site-isolation row.
- **Testing**: the full hierarchy in §27 running in CI, with a growing (not complete) WPT-subset pass rate tracked over time.
- **Web compatibility**: acceptance that this will always trail Chromium/Firefox/WebKit significantly — "production" here means "trustworthy and capable for its intended, bounded use case," not "competes on compatibility with the majors."
- **Performance optimization**: targets in §28 met and re-validated as features are added (feature growth tends to erode performance headroom if not actively managed).

---

## 34. Recommended Implementation Order

The milestone list in §31 *is* the recommended order, but the load-bearing sequencing logic is:

0. **Run both de-risking spikes before writing any layout code, in parallel with each other.** Spike 0's two questions (GPUI Surface interop, Boa-vs-target-corpus viability) are the plan's highest-uncertainty items. If either spike fails, the cost is weeks of throwaway prototype code, not the months it would cost to discover the same failure at M10 or M11 with production code already built on top of the wrong assumption.
1. **Get pixels on screen before anything else is "real" — and get there in two steps, not one.** M1→M10a→M10b is a straight line to "a real webpage renders," with the software-raster checkpoint (M10a) deliberately isolating display-list correctness from GPU-interop risk before M10b introduces it. No detours into JS, storage, or multi-process work happen in this stretch. Nothing in this early sequence is wasted once JS/storage/processes are added later — it's the foundation they attach to.
2. **JS comes after paint, not before — but the JS-engine viability question is already answered by then.** Debugging a JS engine embedding against a browser with no visual output is miserable; M11 deliberately follows M10b so DOM mutations from `<script>` are immediately visible. Because Spike 0(b) already validated Boa against the real target-site corpus, M11 isn't also the milestone where a fundamental engine-choice risk gets discovered.
3. **Storage comes after JS, not before**, because LocalStorage's JS binding is more valuable to build correctly once the JS/DOM binding pattern is already established from `fetch`/timers (M12) — avoids inventing the binding pattern twice.
4. **Process splitting comes last, deliberately.** M14–M16 is the most expensive, most bug-prone phase in the whole roadmap, and doing it against an already-correct single-process engine means bugs found are IPC/process bugs, not "is this feature even correct" bugs conflated together.
5. **Everything from M17 onward is additive breadth**, not architectural risk — Workers, media, devtools, and compatibility/performance/security hardening can be resequenced relative to each other based on actual priorities (e.g., a team that cares more about media than devtools can swap M18/M19) without touching the load-bearing 1→16 sequence.

---

## 35. Final Architecture Diagram

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│                              Windows 11 (Win32/DXGI/MF/DirectWrite)           │
└───────────────────────────────────┬────────────────────────────────────────┘
                                     │
     ┌───────────────────────────────┼─────────────────────────────────┐
     │                     BROWSER PROCESS (owns browser UI + orchestration)│
     │  ┌────────────────────────────────────────────────────────────┐ │
     │  │ GPUI: Windows / Tabs / Omnibox / Toolbar / Menus / Settings  │ │
     │  │        / Downloads UI / History UI / Bookmarks UI            │ │
     │  └───────────────────────┬────────────────────────────────────┘ │
     │                          │ commands/events over `ipc` crate      │
     │  ┌───────────────────────▼────────────────────────────────────┐ │
     │  │ soul-core: WindowMgr / TabMgr (tiered lifecycle) /        │ │
     │  │   NavigationController / SessionMgr / ProfileMgr / Perms     │ │
     │  └──┬───────────────┬───────────────┬───────────────┬─────────┘ │
     │     │               │               │               │           │
     └─────┼───────────────┼───────────────┼───────────────┼───────────┘
           │ IPC            │ IPC           │ IPC           │ IPC
           ▼               ▼               ▼               ▼
  ┌────────────────┐ ┌────────────────┐ ┌────────────┐ ┌────────────────┐
  │ RENDERER PROCESS │ │ NETWORK PROCESS │ │ STORAGE     │ │ GPU PROCESS      │
  │ (per window,      │ │ URL/DNS/TCP/    │ │ SQLite:     │ │ wgpu device,     │
  │  M16+)             │ │ QUIC/TLS/       │ │ cookies/    │ │ compositor,       │
  │                     │ │ HTTP1-3/CORS/CSP│ │ history/    │ │ raster, damage    │
  │ html5ever→DOM       │ └────────────────┘ │ bookmarks/  │ │ tracking, shared   │
  │ cssparser+selectors │                     │ LocalStorage│ │ texture → GPUI     │
  │  → cascade → style  │                     └────────────┘ │ Surface            │
  │ layout (own +taffy) │                                     └────────┬───────────┘
  │ text-shaping        │                                              │
  │  (cosmic-text)       │                                              ▼
  │ paint → display list │                                    DXGI Swapchain → Window
  │ boa JS + web-api     │
  │ (event loop, DOM     │
  │  bindings)           │
  └──────────────────────┘

Legend: solid boxes = OS processes (post-M16). Pre-M16, everything left of the
outermost box runs as threads/tasks inside one process — the arrows above are
identical in meaning either way (§6/§8/ADR-2/ADR-5), which is the whole point.
```

---

*This document is a living plan. Treat §30 (ADRs) as the place decisions get revisited with evidence, not the diagrams — if a diagram and an ADR disagree after a real decision changes, the ADR is the source of truth and the diagram gets updated to match.*
