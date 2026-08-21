# Building a Browser from Scratch in Rust + GPUI on Windows 11
### A Production-Oriented Engineering Plan & Architecture Hub

> **Revision note:** this plan was reviewed after its first draft. The review correctly identified the original 12–18 month MVP timeline as optimistic, flagged the GPUI dependency as an under-weighted project-survival risk (not just a rendering-backend choice), argued the JS-engine compatibility spike needed to move earlier, and pushed for an accessibility semantic tree to be carried from early layout work rather than retrofitted. Memory targets were also revised to be more honest about what Rust actually buys (safety, not automatically-low memory).

> **Implementation status note (2026-08-21):** this plan describes the target architecture; §31 carries the authoritative per-milestone **Status** annotation and status table verified against code. In short: M0–M13, M15, M17, M19, and M20 are **Complete**; M14, M16, M18, and M21–M23 are **Partial** (M14's IPC groundwork is done; the process split itself is deferred per the §31 decision note). Single-crate GPUI isolation is enforced and green CI passes 218 tests across 25 crates.

---

## Quick Navigation & Table of Contents

- **Core Plan & Status**:
  - [§1. Executive Summary](#1-executive-summary)
  - [§2. Design Goals](#2-design-goals)
  - [§3. Non-Goals](#3-non-goals-at-least-through-phase-3)
  - [§4. High-Level Architecture](#4-high-level-architecture)
  - [§5. Complete Architecture Diagram](#5-complete-architecture-diagram)
  - [§24. Repository Structure](#24-repository-structure)
  - [§31. Development Milestones & Status](#31-development-milestones)
  - [§32. MVP Definition](#32-mvp-definition)
  - [§33. Production Definition](#33-production-definition)
  - [§34. Recommended Implementation Order](#34-recommended-implementation-order)
  - [§35. Final Architecture Diagram](#35-final-architecture-diagram)
- **Subsystem Architecture Documents**:
  - [**Core & Process Subsystems** (`docs/subsystems-core.md`)](subsystems-core.md): §6 Process Model, §7 Threading Model, §8 IPC Architecture, §9 GPUI Architecture, §10 Browser Core, §11 Navigation System.
  - [**Engine & Scripting Subsystems** (`docs/subsystems-engine.md`)](subsystems-engine.md): §12 HTML Engine, §13 DOM, §14 CSS Engine, §15 Layout Engine & Text Shaping, §16 Paint System, §17 Compositor, §18 JavaScript Engine.
  - [**Platform & Storage Subsystems** (`docs/subsystems-platform.md`)](subsystems-platform.md): §19 Networking Stack, §20 Storage, §21 GPU Architecture, §22 Windows Platform Layer, §23 Security Architecture.
- **Decisions, Testing & Operations**:
  - [**ADRs & Dependency Strategy** (`docs/adr.md`)](adr.md): §25 Dependency Strategy, §30 Architecture Decision Records (ADR-1 through ADR-19).
  - [**Operations, Testing & Risks** (`docs/operations.md`)](operations.md): §26 Feature Matrix, §27 Testing Architecture, §28 Performance Architecture, §29 Risk Register.

---

## 1. Executive Summary

This is a plan for a real browser engine, not a WebView wrapper. It is written for a **solo-to-small-team developer**, in Rust 1.97.1 / Edition 2024, using **GPUI** for the desktop UI, targeting **Windows 11** first.

The central engineering bet that makes this tractable is: **defer multi-process architecture, keep single-process modularity that is shaped like a future multi-process boundary.** Chromium took hundreds of engineers over a decade to reach site isolation and full sandboxing. A solo developer who tries to build IPC, sandboxing, and crash-isolated multi-process rendering *before* the renderer can lay out a `<div>` will never finish the renderer. Instead, every subsystem is designed behind a message-passing API (commands in, events out) from day one, running in-process on threads/tasks. When the project is mature enough that process isolation is worth the cost, those same APIs move across an OS process boundary with comparatively small changes to call sites, because the *shape* of the interface never changes — only its transport.

This plan is honest about scope: a fully HTML5/CSS3/ES2024-compliant, GPU-accelerated, sandboxed, multi-process browser competitive with Chromium is a **multi-year, likely multi-person** endeavor. What is realistic for one strong systems engineer, working incrementally, is a **usable, GPU-accelerated browser for a well-defined subset of the modern web** (static and moderately dynamic sites, forms, images, basic JS, no video-conferencing-grade media, no extensions), with the architecture below never requiring a rewrite to keep growing after that.

**Timeline, revised:** realistic MVP timeline for a solo/small-team effort is **24–30 months**, with M10 alone plausibly taking 4–6 months once split (see §31) into a software-raster checkpoint and a GPU-compositor milestone.

---

## 2. Design Goals

- **Memory safety first.** `unsafe` is isolated to FFI boundaries (Win32, GPU, font/media libraries) and reviewed as a distinct category of code.
- **Incremental usability.** After every milestone, `soul-shell.exe` should build and let you browse *something* real — even if it's one static HTML page with inline CSS.
- **No rewrite architecture.** Crate boundaries are drawn where process boundaries will eventually go (renderer, network, GPU, storage). Internal APIs are message-shaped, not just function-shaped.
- **Reuse for solved problems, build for the differentiator.** HTML tokenization, TLS, and DNS are solved problems — use mature crates. Layout, style cascade tuned to this engine, tab/process lifecycle, and the compositor integration with GPUI are where the actual engineering work is.
- **Explicit phase boundaries.** Every feature is tagged MVP / Phase 2 / Phase 3 / Advanced / Extremely Difficult, and nothing is implemented out of order without a documented reason.
- **Observability from the start.** Structured logging (`tracing`), crash dumps, and a minimal internal `about:` diagnostics surface exist before M5, because debugging a browser with no visibility into its own state is how these projects die.

---

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
│                         soul-shell.exe                      │
│                                                             │
│  ┌───────────────┐   commands/events   ┌───────────────────┐│
│  │  GPUI (UI     │◄───────────────────►│  soul-core        ││
│  │  thread)      │   (in-proc channel) │  (tabs, nav, state││
│  └───────┬───────┘                     └─────────┬─────────┘│
│          │ Surface texture                       │          │
│          ▼                                       ▼          │
│  ┌───────────────┐                      ┌───────────────────┐│
│  │  compositor   │◄──display lists──────│  renderer         ││
│  │  (wgpu)       │                      │  (html/css/dom/   ││
│  └───────┬───────┘                      │   layout/js)      ││
│          │                              └─────────┬─────────┘│
│          ▼                                        │          │
│      GPU / DXGI                         ┌─────────▼────────┐│
│                                         │  networking(tokio││
│                                         │  (http/tls/dns)  ││
│                                         └─────────┬─────────┘│
│                                         ┌─────────▼────────┐│
│                                         │  storage (sqlite)││
│                                         └──────────────────┘│
└─────────────────────────────────────────────────────────────┘

Phase 2 (M14+): Split Processes
┌───────────────┐   IPC (named pipe +   ┌───────────────┐
│ Browser proc  │◄──framed protocol)───►│ GPU process   │
│ GPUI + core   │                       │ wgpu/D3D12    │
└──────┬────────┘                       └───────────────┘
       │ IPC                                    ▲
       ▼                                         │ shared texture (DXGI)
┌───────────────┐   IPC         ┌───────────────┐
│ Renderer proc │◄─────────────►│ Network proc  │
│ (per window)  │               │ HTTP/TLS/DNS  │
└───────────────┘               └───────────────┘
```

The critical property: **the arrows above don't change meaning** between Phase 1 and Phase 2 — only whether the arrow is a Rust channel or a named pipe.

---

## 5. Complete Architecture Diagram

```text
                                   ┌────────────────────────────┐
                                   │        Windows 11 OS       │
                                   │ Win32 / DXGI / MF / DWrite │
                                   └───────────────┬────────────┘
                                                   │
                     ┌─────────────────────────────┼─────────────────────────────┐
                     │                    BROWSER PROCESS                        │
                     │                                                           │
   ┌─────────────┐   │   ┌───────────────┐    ┌───────────────┐    ┌───────────┐ │
   │ Input       │──►│──►│ GPUI Shell    │───►│ Input Router  │───►│ Hit Test/ │ │
   │ (mouse/kb)  │   │   │ Windows/Tabs/ │    │               │    │ Focus     │ │
   │             │   │   │ Omnibox/Menus │    └───────┬───────┘    └─────┬─────┘ │
   └─────────────┘   │   └───────┬───────┘            │                  │       │
                     │           │ commands           │ routed input     │       │
                     │           ▼                    ▼                  ▼       │
                     │   ┌────────────────────────────────────────────────────┐  │
                     │   │                    soul-core                       │  │
                     │   │ Window Mgr │ Tab Mgr │ Navigation │ Session        │  │
                     │   │ Profile │ Permission Mgr │ History │ Bookmarks     │  │
                     │   └──────┬──────────────────────┬──────────────┬───────┘  │
                     │          │ page commands        │ net requests │ storage  │
                     │          ▼                      ▼              ▼          │
                     │   ┌───────────────┐    ┌────────────────┐ ┌──────────┐    │
                     │   │ renderer(s)   │    │ networking     │ │ storage  │    │
                     │   │ HTML/CSS/DOM/ │    │ DNS/TCP/QUIC/  │ │ SQLite/  │    │
                     │   │ Layout/JS     │    │ TLS/HTTP1-3    │ │ Cache    │    │
                     │   └──────┬────────┘    └────────────────┘ └──────────┘    │
                     │          │ display list                                   │
                     │          ▼                                                │
                     │   ┌───────────────┐                                       │
                     │   │ compositor    │                                       │
                     │   │ (wgpu)        │                                       │
                     │   └──────┬────────┘                                       │
                     │          │ shared surface texture                         │
                     └──────────┼────────────────────────────────────────────────┘
                                ▼
                        ┌────────────────┐
                        │ GPU process /  │
                        │ in-proc wgpu   │
                        │ device (DX12)  │
                        └───────┬────────┘
                                ▼
                         DXGI Swapchain → Window
```

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
│   ├── soul-ui/             # `SoulBackend` trait + backend-agnostic view logic
│   ├── soul-backend-gpui/    # concrete `SoulBackend` impl against GPUI (ONLY crate with gpui)
│   ├── ipc/                    # command/event message types + channel/named pipe transports
│   ├── html/                   # html5ever TreeSink impl → this project's DOM
│   ├── dom/                    # arena-based DOM, NodeId, mutation API
│   ├── css/                    # cssparser/selectors integration, cascade, computed style
│   ├── layout/                 # box generation, block/inline layout, taffy flexbox
│   ├── text-shaping/           # cosmic-text/rustybuzz/fontdb/DirectWrite integration
│   ├── paint/                  # display list types + builder
│   ├── raster/                 # tiny-skia CPU raster backend
│   ├── compositor/             # wgpu compositing, tiling, damage tracking
│   ├── javascript/             # boa embedding, event loop, GC integration
│   ├── web-api/                # DOM bindings, rich fetch/Promise/timers/IndexedDB/Worker
│   ├── networking/              # url/DNS/TCP/QUIC/TLS/HTTP1-3/cookies/CORS/CSP
│   ├── storage/                # SQLite-backed cookie/history/bookmarks/LocalStorage/cache
│   ├── image-decode/            # image/resvg integration, background decode pool
│   ├── media/                  # Media Foundation bindings for <audio>/<video>
│   ├── gpu/                    # wgpu device/surface management
│   ├── platform-windows/        # Win32 wrappers: shell execute, MOTW, UIA, registry
│   ├── sandbox/                # (Phase 2+) Job Objects, restricted tokens, AppContainer
│   ├── downloads/               # download manager (HTTP stream + MOTW + collision)
│   ├── devtools/                # (Phase 2+) CDP server / inspector backend
│   └── common/                  # shared types, error types, tracing setup
├── resources/                    # default icons, error pages, UA stylesheet
├── tests/                        # workspace integration + web-platform-test harness
├── benchmarks/                   # criterion benches: layout, paint, parse
└── docs/                         # architecture plan, ADRs, modular subsystem specs
```

**Dependency direction** (enforced by workspace lint / metadata check):
`soul-shell` → `soul-backend-gpui` → `soul-ui` (trait only) / `soul-core` → `ipc` → {`html`, `css`, `dom`, `layout`, `javascript`, `networking`, `storage`, `compositor`} → {`gpu`, `text-shaping`, `raster`, `image-decode`, `media`, `platform-windows`} → `common`.

---

## 31. Development Milestones

**Status key (as of 2026-08-18, verified against repository code):**
- **Complete** — the milestone's components exist, are wired, and pass all workspace tests.
- **Partial** — the milestone's components exist but lack key functionality, live process splits, or external decoders.

| Milestone | Plan intent | Repo status |
|---|---|---|
| Spike 0 | GPUI + Boa de-risking (ADR-1, ADR-4) | **Complete** |
| M0 Foundation | 25-crate workspace, tracing, CI | **Complete** |
| M1 GPUI shell | real window via `SoulBackend` | **Complete** |
| M2 Window + input | OS input events, DPI | **Complete** |
| M3 URL + navigation | nav state machine, stub fetch | **Complete** |
| M4 Networking | HTTP(S), DNS, redirects, cookies, Brotli/Zstd, HSTS | **Complete** — streaming decompression (`gzip`, `deflate`, `br`, `zstd`), persistent SQLite RFC 6797 HSTS policy auto-upgrades |
| M5 HTML parser | html5ever → DOM | **Complete** |
| M6 DOM API | NodeId arena, mutation, query, MutationObserver | **Complete** |
| M7 CSS + style | cascade, computed style, custom properties (`var()`), pseudo-elements | **Complete** — supports `var(--name)`, pseudo-elements (`::before`, `::after`, `::placeholder`, etc.), `content` property |
| M8 Layout | block/inline/flex + text shaping | **Partial** — block and inline layout are wired and tested; `display: flex` containers dispatch to taffy; generated pseudo-element boxes (`style.content`) generate layout nodes; remaining gaps: text shaping uses synthetic-advance stub (ADR-18); em/rem/calc resolution; absolute/fixed positioning |
| M9 Paint | display list | **Complete** |
| M9.5 A11y skeleton | semantic data in fragment tree | **Complete** |
| M10a Software raster | CPU pixels on screen | **Complete** |
| M10b GPU compositor | wgpu compositor | **Complete** |
| M11 Basic JS | boa + event loop + DOM bindings | **Complete** |
| M12 Web APIs | fetch, timers, Promises, Crypto, URL, Performance | **Complete** — Web Crypto (`crypto.randomUUID()`, `crypto.getRandomValues()`), WHATWG `URL`/`URLSearchParams`, High-Res Time (`performance.now()`, `requestAnimationFrame()`) |
| M13 Storage | SQLite persistence, HSTS, DPAPI | **Complete** |
| M14 GPU split + IPC | GPU process, real IPC | **Partial** — IPC transport (in-memory + named pipe), framing codec, and message contracts are complete and tested; the GPU-process split itself remains deferred (see note below) |
| M15 Network split | networking over IPC | **Complete** — the live browser path (`soul-shell` navigation driver) routes all network traffic through `BrowserToNetworkMsg`/`NetworkToBrowserMsg` via a pluggable `NetworkClient` over the in-memory transport, with mixed-content/CORS enforcement in the network service, request cancellation on both transports, per-request timeouts, and the named-pipe transport proven end-to-end; the cross-process flip is now a constructor change |
| M16 Sandboxing | renderer process, Job Objects | **Partial** |
| M17 Advanced Web APIs | Workers, IndexedDB, richer fetch, Canvas 2D, WebSockets | **Complete** — `WebWorker` (thread + mpsc + 2nd VM), SQLite `IndexedDbStore`, Boa `window.indexedDB`, rich `fetch` (`Headers`, `Request`, `Response`, body readers `text()`/`json()`/`arrayBuffer()`), Canvas 2D, WebSockets |
| M18 Media | MF playback + Canvas 2D | **Partial** |
| M19 DevTools | inspector/console/network CDP | **Complete** |
| M20 Downloads | download manager + MOTW | **Complete** |
| M21 Compositor + perf | damage/layers, benchmark harness | **Partial** |
| M22 Security | CSP, DPAPI, HSTS, private browsing | **Partial** |
| M23 Production | signed installer, updates, crash reporting | **Partial** |

**Known deviations from plan intent (audit 2026-08-21, tracked in ADR-17..19):**
- Text is drawn as placeholder solid rectangles — no glyph shaping or rasterization yet (M8 Partial).
- CSS tokenizer/selector matching is hand-rolled; `cssparser`/`selectors` reuse deferred (ADR-17).
- Text shaping is a synthetic-advance stub; `cosmic-text`/`rustybuzz` integration outstanding (ADR-18).
- DNS uses the platform resolver via `tokio`, not `hickory-resolver`; cookie parsing is hand-rolled rather than the `cookie` crate (ADR-19).
- CSP module exists but is not yet enforced on any response path (M22 Partial).
- Cookie jar: SameSite not enforced on send, no public-suffix enforcement, `Expires` attribute ignored (ADR-19).

**Note (current wiring work):** `soul-shell` has a fully connected path — `NavigationController` drives live HTTP(S) fetches through the full rendering pipeline, with per-stage timings, PNG screenshot output, inline and external `<script src="...">` execution with Web Storage, rich `fetch()` (`Headers`, `Request`, `Response`) and DOM mutations, external `<link rel="stylesheet">` CSS parsing and cascade resolution, accessibility-tree extraction (verified against live sites and fixtures), and presentation in a genuine native GPUI window with tab switching, URL input, retained scrolling, and dynamic resizing.

---

### Next Steps (as of 2026-08-21)

Seven milestones remain Partial or re-flagged: **M8** (Layout & Text Shaping), **M14**, **M16**, **M18**, **M21**, **M22**, **M23**.

**M14/M15 decision:** keep explicit single-process execution in `soul-shell` while hardening the message-shaped core. Rationale: the engine is still evolving (M18/M21/M22/M23), the named-pipe transport is already proven, and flipping the boundary now would add process-lifecycle failure modes on top of active engine work. The hardening work (M15) is done: the IPC network contract is complete (bodies, security context, final URL, set-cookies), the service enforces mixed content/CORS and honors cancellation on both transports, and `soul-shell`'s live browser path runs through `NetworkClient` over the in-memory transport — the process split is now a transport-swap (ADR-2/ADR-5). The remaining M14 work is the GPU-process split (shared-texture interop), which stays deferred.

**Verified Ground Truth Baseline (Completed 2026-08-21):**
```
cargo fmt --all -- --check                             # CLEAN (0 diffs)
cargo clippy --workspace --all-targets -- -D warnings   # CLEAN (0 warnings across all 25 crates)
cargo test --workspace                                # CLEAN (218 passed, 0 failed)
```
Single-crate GPUI boundary verified: `soul-backend-gpui` is the only workspace crate depending on `gpui` (upstream `gpui_*` subcrates of the zed git dependency excluded).

**Milestone Execution Sequence:**
1. **M8 / M10** — implement real glyph shaping and rasterization via `cosmic-text`/`rustybuzz` and DirectWrite system fonts.
2. **M8** — implement CSS relative units (`em`, `rem`, `vw`, `vh`, `calc()`) and `position: absolute`/`fixed` layout.
3. **M22** — implement private/incognito ephemeral storage profile isolation and full CSP directive enforcement.
4. **M18** — implement real Media Foundation decode in `crates/media/src/` via COM `IMFSourceReader`.
5. **M16** — apply `RestrictedToken` to spawned processes and launch a live child into `JobObject`.
6. **M23** — implement update manifest fetching, binary download, signature verification against `DpapiVault`/public keys, and real installer.

---

## 32. MVP Definition

The MVP is reached at the end of **M13**. It can:
- Open URLs typed into an omnibox over HTTP and HTTPS (with TLS validation).
- Perform DNS resolution, HTTP/1.1 and HTTP/2 requests with redirects, decompression, and cookies.
- Parse HTML into a DOM (via `html5ever`).
- Parse CSS for MVP properties and compute styles via cascade.
- Render CSS box model, block/inline flow, positioning, colors, fonts, backgrounds, borders.
- Display text (glyph rendering outstanding — currently placeholder rectangles; `cosmic-text` integration is the remaining M8 work, ADR-18) and images (via `image`).
- Scroll smoothly, decoupled from layout.
- Follow links and navigate with a back/forward stack.
- Run multiple tabs in GPUI browser UI.
- Execute basic JavaScript (DOM manipulation, event listeners, `setTimeout`, rich `fetch`).
- Enforce baseline security (HTTPS, SOP, CORS).
- Persist cookies, history, and bookmarks across restarts.

---

## 33. Production Definition

Before calling this a "production browser," it needs (beyond MVP, cumulative through M23):
- **Modern HTML**: Shadow DOM, `<template>`, full table layout, custom elements lifecycle.
- **Modern CSS**: Flexbox, Grid, animations/transitions, container queries, `:has()`, custom properties.
- **JavaScript & Web APIs**: ECMAScript coverage, Promises/async, Workers, WebAssembly, IndexedDB, rich Fetch, MutationObserver.
- **GPU acceleration**: full compositor path (§17/§21), GPU damage tracking.
- **Multi-process architecture**: GPU, network, and renderer-per-window processes (§6).
- **Sandboxing**: Job Object + restricted token + AppContainer renderer sandboxing (§22/§23).
- **Storage & Security**: full LocalStorage/IndexedDB/Cache Storage, quotas, DPAPI-encrypted secrets, private browsing, CSP enforcement.
- **Media**: common audio/video playback via Media Foundation, Canvas 2D.
- **Accessibility**: UI Automation provider implementation bridging `A11yNode` tree.
- **Developer tools**: Elements, Console, Network, Storage panels.
- **Automatic updates**: signed update mechanism & installer (M23).

---

## 34. Recommended Implementation Order

0. **De-risking spikes** (GPUI Surface interop, Boa-vs-target-corpus viability) — completed.
1. **Get pixels on screen** (M1→M10a→M10b) — software raster checkpoint before GPU compositing.
2. **JavaScript after paint** (M11→M12) — DOM mutations are visually verifiable.
3. **Storage after JS** (M13) — LocalStorage bindings follow established Web API patterns.
4. **Process splitting deliberately staged** (M14→M16) — isolate stable single-process subsystems.
5. **Additive breadth** (M17→M23) — rich Web APIs, Media, DevTools, Security, and Production release.

---

## 35. Final Architecture Diagram

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│                              Windows 11 (Win32/DXGI/MF/DirectWrite)          │
└───────────────────────────────────┬──────────────────────────────────────────┘
                                    │
     ┌──────────────────────────────┼─────────────────────────────────┐
     │                     BROWSER PROCESS (owns browser UI + orchestration)│
     │  ┌────────────────────────────────────────────────────────────┐ │
     │  │ GPUI: Windows / Tabs / Omnibox / Toolbar / Menus / Settings │ │
     │  │        / Downloads UI / History UI / Bookmarks UI          │ │
     │  └───────────────────────┬────────────────────────────────────┘ │
     │                          │ commands/events over `ipc` crate    │
     │  ┌───────────────────────▼────────────────────────────────────┐ │
     │  │ soul-core: WindowMgr / TabMgr (tiered lifecycle) /         │ │
     │  │   NavigationController / SessionMgr / ProfileMgr / Perms   │ │
     │  └──┬───────────────┬───────────────┬───────────────┬─────────┘ │
     │     │               │               │               │           │
     └─────┼───────────────┼───────────────┼───────────────┼───────────┘
           │ IPC           │ IPC           │ IPC           │ IPC
           ▼               ▼               ▼               ▼
  ┌────────────────┐ ┌────────────────┐ ┌────────────┐ ┌────────────────┐
  │ RENDERER PROC  │ │ NETWORK PROC   │ │ STORAGE    │ │ GPU PROCESS    │
  │ (per window,   │ │ URL/DNS/TCP/   │ │ SQLite:    │ │ wgpu device,   │
  │  M16+)         │ │ QUIC/TLS/      │ │ cookies/   │ │ compositor,    │
  │                │ │ HTTP1-3/CORS/  │ │ history/   │ │ raster, damage │
  │ html5ever→DOM  │ │ CSP            │ │ bookmarks/ │ │ tracking,      │
  │ cssparser+sel. │ └────────────────┘ │ LocalStor. │ │ shared texture │
  │ layout+taffy   │                    └────────────┘ │ → GPUI Surface │
  │ cosmic-text    │                                   └────────┬───────┘
  │ paint→disp-list│                                            │
  │ boa JS+web-api │                                            ▼
  └────────────────┘                                   DXGI Swapchain → Window
```
