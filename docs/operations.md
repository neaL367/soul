# Operations: Feature Matrix, Testing, Performance & Risks

This document contains **Feature Matrix, Testing Architecture, Performance Budgets, and the Risk Register** (§26–§29) for the Soul Browser Engine.
For the main architecture index and milestone status, see [`docs/architecture-plan.md`](architecture-plan.md).

---

## 26. Feature Matrix

Legend: **M**=MVP, **R**=Required (ships shortly after MVP), **L**=Later/Phase 2-3, **A**=Advanced, **X**=Extremely Difficult, **O**=Optional/may never ship, **✓**=Implemented & Verified.

### Browser Core
| Feature | Class | Feature | Class |
|---|---|---|---|
| Tabs (open/close/switch) | ✓ | Session restore | ✓ |
| Windows (multi-window) | ✓ | Bookmarks | ✓ |
| Navigation (URL bar, back/fwd) | ✓ | Downloads (resumable + MOTW) | ✓ |
| Search (omnibox → search engine) | ✓ | Tab freeze/discard (memory tiering) | ✓ |
| History | ✓ | Sync / multi-device | O |

### HTML
| Feature | Class | Feature | Class |
|---|---|---|---|
| Parser (html5ever) | ✓ | Forms (basic inputs) | ✓ |
| DOM tree (NodeId arena) | ✓ | Tables (structural) | ✓ |
| Links/navigation | ✓ | Custom elements (parse-level) | L |
| Images | ✓ | Shadow DOM | A |
| Metadata (`<meta>`, `<title>`) | ✓ | Full table layout algorithm | A |

### CSS
| Feature | Class | Feature | Class |
|---|---|---|---|
| Selectors (basic + combinators) | ✓ | Grid | ✓ |
| Cascade/specificity | ✓ | Media queries (`@media`, `matchMedia`) | ✓ |
| Box model | ✓ | Relative Units (`em`, `rem`, `vw`, `vh`, `calc()`) | ✓ |
| Positioning (`static`, `relative`, `absolute`, `fixed`) | ✓ | Transforms (2D) | L |
| Flexbox (taffy) | ✓ | Transitions | L |
| Pseudo-Elements (`::before`, `::after`, `content`) | ✓ | Custom Properties (`var()`) | ✓ |
| `:has()`, container queries | A | Animations (`@keyframes`) | A |
| Print/pagination CSS | X | | |

### JavaScript & Web APIs
| Feature | Class | Feature | Class |
|---|---|---|---|
| ECMAScript core (via boa) | ✓ | Web Workers (2nd VM + mpsc) | ✓ |
| Basic DOM APIs | ✓ | IndexedDB (`window.indexedDB`) | ✓ |
| Events & Event Loop | ✓ | Canvas 2D (`CanvasRenderingContext2D`) | ✓ |
| `fetch` (rich Headers/Request/Response) | ✓ | WebSockets (`WebSocket`) | ✓ |
| Promises/async-await | ✓ | Web Crypto (`randomUUID`, `getRandomValues`) | ✓ |
| WHATWG `URL` & `URLSearchParams` | ✓ | High-Res Time (`performance.now`, `rAF`) | ✓ |
| WHATWG `FormData` | ✓ | W3C `Blob` & `File` APIs | ✓ |
| CSSOM View (`window.matchMedia`) | ✓ | HTML5 `<video>` / `<audio>` (`HTMLMediaElement`) | ✓ |
| `MutationObserver` | ✓ | WebAssembly | A |
| LocalStorage & SessionStorage | ✓ | Service Workers | X |

### Storage & Security
| Feature | Class | Feature | Class |
|---|---|---|---|
| Cookies (RFC 6265bis SameSite/Secure) | ✓ | HTTPS/TLS (rustls) | ✓ |
| LocalStorage | ✓ | Same-Origin Policy & CORS | ✓ |
| SessionStorage (in-memory) | ✓ | Brotli & Zstd Decompression | ✓ |
| IndexedDB | ✓ | RFC 6797 HSTS Policies | ✓ |
| HTTP cache | ✓ | DPAPI Vault (`CryptProtectData`) | ✓ |
| CSP Level 3 Active Enforcement | ✓ | Private Browsing Profile Isolation | ✓ |
| Sandboxing (Job Objects + Tokens) | L | Full Chromium sandbox parity | X |

### Developer Tools
| Feature | Class | Feature | Class |
|---|---|---|---|
| Chrome DevTools Protocol (CDP) | ✓ | Sources/debugger | A |
| Console domain | ✓ | Performance profiler | A |
| Elements / DOM domain | ✓ | Storage inspector | L |
| Network domain | ✓ | | |

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
| Memory (background tabs) | Materially lower than active tabs | The tab-tiering system (Hot/Warm/Cold/Frozen) is the direct lever here — see §10 tab lifecycle |
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
| GPU rendering complexity (wgpu/DXGI interop with GPUI) | Medium-High | High (blocks everything downstream) | Medium | Resolve the GPUI-backend ADR (§30, ADR-1) *before* M1; prototype the Surface-texture handoff as a spike before committing |
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
