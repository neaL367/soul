# Subsystems: Rendering Engine & Scripting

This document contains detailed architecture specifications for the **HTML Engine, DOM, CSS Engine, Layout, Paint, Compositor, and JavaScript Engine** (§12–§18) of the Soul Browser Engine.
For the main architecture index and milestone status, see [`docs/architecture-plan.md`](file:///d:/Hobby/soul/docs/architecture-plan.md).

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

---

## 13. DOM

A from-scratch DOM crate, because layout, style, and JS bindings all need to walk it with different access patterns and this project's DOM has no reason to carry Servo's Stylo-specific baggage.

- **Storage:** arena-allocated (`slotmap` or a hand-rolled generational arena) node pool, not `Rc<RefCell<Node>>` trees — avoids reference-cycle/borrow-checker pain and is dramatically more cache-friendly for layout traversal.
- **Node identity:** `NodeId` (generational index) is the currency passed between HTML parser, style system, layout, and JS bindings — never raw pointers, keeping everything `Send` where needed for future multi-threaded style/layout.
- **Mutation:** DOM mutations (from parsing *or* from JS) go through one mutation API that also records the invalidation needed for style/layout (dirty bits), so "JS calls `appendChild`" and "parser inserts a node" hit the same invalidation path — no special-casing that could get them out of sync.
- **MVP:** element/text/comment/document nodes, attributes, basic tree mutation API (`appendChild`, `removeChild`, `setAttribute`), `querySelector`/`querySelectorAll` (via the `selectors` crate, shared with CSS matching — see §14).
- **Phase 2:** `MutationObserver`-equivalent internal event stream (needed before JS `MutationObserver` API can exist), Shadow-DOM-aware tree walking.
- **Advanced:** full Shadow DOM encapsulation semantics, slot assignment.

---

## 14. CSS Engine

**Decision: reuse `cssparser` + `selectors`** (both Servo-maintained, both genuinely low-level and reusable independent of Stylo) for tokenizing and selector matching. **Write the cascade, computed-value resolution, and layout tree from scratch**, tuned to this engine's DOM and to `taffy` (see §15) as the box-layout solver.

> *Implementation status (2026-08-18):* the current `css` crate hand-rolls tokenizing/selector matching for the MVP property subset; `cssparser`/`selectors` reuse is deferred — see ADR-17.

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

### Text shaping (own subsystem, reused libraries)

`cosmic-text` (which bundles `rustybuzz` for shaping — a Rust port of HarfBuzz — plus `swash` for glyph rasterization and `fontdb` for font matching/fallback) is the recommended reuse target: writing a correct Unicode line-breaker + bidi + shaping engine from scratch is an "extremely difficult" bucket item on its own and has essentially zero differentiation value for a browser project. System font enumeration on Windows goes through **DirectWrite** (`windows` crate bindings) feeding `fontdb`.

> *Implementation status (2026-08-18):* `text-shaping` currently ships a synthetic-advance stub and text renders as placeholder rectangles — see ADR-18; `cosmic-text` integration is the remaining M8 work.

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

---

## 17. Compositor

- Built on **`wgpu`** (Vulkan or D3D12 backend on Windows, chosen by `wgpu` at device-creation time — see ADR-6, §30). `wgpu` is a safe, actively maintained, cross-platform GPU abstraction; hand-rolling raw Vulkan or D3D12 command buffer management is a large, security-relevant `unsafe` surface with no payoff versus `wgpu` for this project's needs (2D-heavy compositing, not a game engine).
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
