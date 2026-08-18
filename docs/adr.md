# Architecture Decision Records & Dependency Strategy

This document contains **Dependency Strategy** (§25) and all **Architecture Decision Records (ADRs)** (§30) for the Soul Browser Engine.
For the main architecture index and milestone status, see [`docs/architecture-plan.md`](file:///d:/Hobby/soul/docs/architecture-plan.md).

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

**ADR-17: CSS parsing implementation deviation (audit-flagged 2026-08-18)**
- *Decision:* The `css` crate currently tokenizes and matches selectors with a hand-rolled parser (a subset of css-syntax rules: declarations, `!important`, simple `.class`/`#id`/tag/descendant selectors) instead of reusing `cssparser` + `selectors` as ADR-3 specifies. The cascade and computed-value resolution remain this project's own code, as planned. Reuse of `cssparser`/`selectors` is deferred and must precede any expansion of CSS grammar coverage (custom properties, media queries, combinator breadth).
- *Rationale:* The hand-rolled parser shipped as the fastest path to M7 (cascade/computed style) and was sufficient for the MVP property subset; a 2026-08-18 full-workspace audit confirmed it is the only reusable-parser deviation in the workspace.
- *Consequence:* §31 M7 stays **Complete** for cascade functionality but carries a deviation note; ADR-3's security rationale (mature, fuzzed parsers for untrusted input) applies with increased force as the grammar grows — parser swap is tracked, not abandoned.

**ADR-18: Text shaping implementation deviation (audit-flagged 2026-08-18)**
- *Decision:* `text-shaping` currently provides a synthetic-advance shaper stub (font-independent advance widths, no glyph ids) and the live raster path (`soul-shell` engine stages) paints text as placeholder solid rectangles — no glyph rasterization exists. `cosmic-text`/`rustybuzz`/`fontdb` integration (per §15/ADR table) is the outstanding M8 work.
- *Rationale:* Block/inline layout needed width estimates before a real shaper existed; the placeholder kept the pipeline testable. Real glyph rendering is feature work, not audit-fix scope.
- *Consequence:* §31 M8 re-flagged **Partial**; the MVP bullet "display text" (architecture-plan §32) amended to reflect placeholder rectangles until cosmic-text lands.

**ADR-19: DNS and cookie-parsing reuse deviations (audit-flagged 2026-08-18)**
- *Decision:* DNS uses the platform resolver through `tokio::net::lookup_host` rather than `hickory-resolver` (per §19/ADR table); cookie parsing in `storage::cookies` is hand-rolled rather than the `cookie` crate. Known consequences of the hand-rolled jar, recorded for the M22 Security milestone: SameSite is stored but not enforced when sending; no public-suffix list enforcement (`Domain=.com` can poison all `.com` hosts); `Expires` attribute is ignored (only `Max-Age`); IP-address domain suffixes can match partial octets.
- *Rationale:* Platform resolver removed a `hickory-resolver` dependency at build time; the cookie parser shipped with the M4/§31 status-verified baseline. Both deviations are reuse-table violations that carry security implications at the edges.
- *Consequence:* Tracked against M22 Security; ADR-table rows for DNS and cookie parsing are amended by this record. Enforcement gaps (SameSite/PSL) are fixes queued for the security milestone rather than silent plan updates.
