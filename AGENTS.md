# AGENTS.md — Rust/GPUI Browser Engine

This file is the operational contract for any AI coding agent working in this repository. It is authoritative for *how to work*. The deep technical rationale — full architecture, ADRs, feature matrix, risk register, milestone details — lives in `docs/architecture-plan.md` and is authoritative for *what to build and why*. Where the two disagree, `docs/architecture-plan.md` wins for architecture/design questions and this file wins for process/workflow questions; if they genuinely conflict on a design point, stop and flag it rather than picking one silently — that's a sign the plan needs a new ADR, not a sign to guess.

Section numbers below (`§N`) refer to `docs/architecture-plan.md` unless stated otherwise. References to this file are written explicitly as `AGENTS.md §N`; references to the architecture plan are written as `docs/architecture-plan.md §N` to prevent cross-document ambiguity.

---

## 0. Current Project State — read this first

**This project may still be pre-code.** Before doing anything else, check whether `Cargo.toml` exists at the workspace root.

- **If it does not exist:** the project is at or before **M0 (Project Foundation)**. Your task is almost certainly bootstrapping work. Do not invent a different structure — scaffold exactly the crate layout in §24, with each crate as an empty-but-compiling stub (`lib.rs` with a doc comment stating its future responsibility, no speculative code). Do not implement M5+ functionality inside an M0 task just because it seems natural to "get ahead" — see AGENTS.md §11 Change-Scope Discipline.
- **If it exists:** find the furthest-completed milestone by checking `docs/architecture-plan.md` §31 against what's actually implemented (don't trust a stale changelog — verify against code). Confirm which milestone your task belongs to, and confirm its stated dependencies (the "Depends on" field per milestone) are actually satisfied before starting. If asked to implement something whose milestone dependency isn't done yet, say so instead of implementing it out of order.
- **Spike 0 status matters disproportionately.** Two architectural questions are load-bearing for everything downstream: (a) which GPUI integration path (ADR-1) — is `soul-ui`'s `SoulBackend` trait implemented against mainline GPUI or the `gpui-ce`/wgpu fork? (b) is Boa confirmed viable against the target-site JS corpus, or has a pivot to `rquickjs` already happened (ADR-4)? Check for a `docs/spike-0-results.md` or equivalent before assuming either answer. If it doesn't exist and you're asked to do JS-engine or GPUI-integration work, that's a signal the spike itself is the actual missing prerequisite — flag it rather than guessing an answer and building on top of the guess.

---

## 1. Project Mission and Scope

**What this is:** a real browser engine and browser application, built from scratch in Rust 1.97.1 (Edition 2024), with GPUI-based browser UI, targeting Windows 11. Not a WebView2/Chromium/Electron wrapper — there is no embedded complete browser engine anywhere in this codebase.

**What it's trying to achieve:** a genuinely usable browser for a well-defined subset of the modern web (static-to-moderately-dynamic sites: documentation, blogs, forms, images, basic interactive JS), built on an architecture that never requires a rewrite to keep growing toward broader compatibility later. See `docs/architecture-plan.md` §1 (Executive Summary) and §32 (MVP Definition) for the precise current target.

**Explicitly out of scope** (do not implement, and question any task that implies otherwise — see §3 for the full list): browser extensions, DRM/EME, mobile or non-Windows platforms, a sync service, telemetry pipeline, a JS JIT, Chromium-parity sandboxing, full site isolation. These are not "not yet" in the sense of "someday soon" for most of them — some are permanent non-goals for this project. Check §3 before assuming something is merely deferred.

**Core engineering philosophy** (this is the thing to internalize, not just follow mechanically): **defer multi-process/sandboxing, but shape every internal API as if the process boundary already existed.** Every cross-subsystem interface is a command/event message (§8, ADR-5), even while it currently runs as an in-process function call or channel send. This is the single rule that makes "no rewrite architecture" (§2) actually true. If you write a new cross-crate interface as a direct trait-method call with no message-shaped equivalent, you are working against this project's central architectural bet — restructure it as commands/events even before Phase 2 IPC exists.

---

## 2. Non-Negotiable Constraints

| Constraint | Value | Notes |
|---|---|---|
| Language | Rust 1.97.1, Edition 2024 | Pinned in `rust-toolchain.toml`. Do not bump without an explicit request — this is a stated project constraint, not a default. |
| UI framework | GPUI, behind the `SoulBackend` trait | Never import `gpui::*` outside `soul-backend-gpui`. This is enforced, not just recommended — see AGENTS.md §3. |
| Platform | Windows 11 only | No `#[cfg(target_os = "macos")]`/`linux` branches unless explicitly requested; don't add cross-platform abstraction "for the future" unprompted (AGENTS.md §5, Rules for introducing abstractions). |
| Forbidden | Electron, WebView2, any embedded complete browser engine (CEF, Servo-as-a-whole, etc.), Node.js runtime, .NET runtime | If a task seems to require one of these to be "easy," that's a signal to reconsider the approach, not to add the dependency. |
| Dependency philosophy | Reuse solved problems, build the differentiator from scratch | See AGENTS.md §6 (Dependency Policy) and `docs/architecture-plan.md` §25 for the exact per-subsystem table. Security-critical primitives (crypto/TLS) are never reimplemented. |
| Memory safety | `unsafe` confined to well-known FFI boundaries (Win32, GPU, font/media libs) | New `unsafe` outside those boundaries needs a specific justification in the PR/commit description, not just "it was convenient." |
| Process model | Single process through M13; GPU/network/renderer splits at M14–M16; site isolation explicitly deferred | Don't add process-splitting code before its milestone — see AGENTS.md §0 and §11. |
| Performance targets | See `docs/architecture-plan.md` §28 | These are goals to design around, not gates that block every commit — don't over-index on hitting a number at the expense of correctness during early milestones. |

---

## 3. Repository Structure

Canonical target layout — reproduce exactly, don't improvise a "better" structure:

```text
browser/
├── Cargo.toml, Cargo.lock, rust-toolchain.toml
├── crates/
│   ├── soul-shell/        bin — entry point, wires everything together
│   ├── soul-core/         tab/window/nav/session/profile/permission state machines
│   ├── soul-ui/           SoulBackend trait + backend-agnostic browser-UI view logic
│   ├── soul-backend-gpui/  the ONLY crate allowed to depend on `gpui`
│   ├── ipc/                  command/event message types + Phase-1/Phase-2 transports
│   ├── html/                 html5ever TreeSink impl → dom
│   ├── dom/                  arena-based DOM, NodeId, mutation API
│   ├── css/                  cssparser/selectors integration, cascade, computed style
│   ├── layout/               box generation, block/inline layout, taffy integration
│   ├── text-shaping/         cosmic-text/rustybuzz/fontdb/DirectWrite integration
│   ├── paint/                display list types + builder
│   ├── raster/               tiny-skia CPU raster backend
│   ├── compositor/           wgpu compositing, tiling, damage tracking
│   ├── javascript/           boa embedding, event loop, GC integration
│   ├── web-api/              DOM bindings, fetch/Promise/timers
│   ├── networking/           url/DNS/TCP/QUIC/TLS/HTTP1-3/cookies/CORS/CSP
│   ├── storage/              SQLite-backed cookies/history/bookmarks/LocalStorage/cache
│   ├── image-decode/         image/resvg integration
│   ├── media/                Media Foundation bindings
│   ├── gpu/                  wgpu device/surface management
│   ├── platform-windows/     Win32 wrappers not owned by GPUI
│   ├── sandbox/              (Phase 2+) Job Objects, tokens, AppContainer
│   ├── downloads/            download manager
│   ├── devtools/             (Phase 2+) inspector backend + UI
│   └── common/                shared types, error types, tracing setup
├── resources/    default icons, error pages, UA stylesheet
├── tests/        workspace integration + WPT-subset harness
├── benchmarks/   criterion benches
└── docs/         architecture-plan.md, ADRs, per-crate design notes
```

**Where things belong:**
- Application/business logic → `soul-core`. Browser-UI view logic (backend-agnostic) → `soul-ui`. GPUI-specific code → `soul-backend-gpui` **only**.
- Anything touching DOM/CSS/layout/paint/JS → the matching crate above; these must never depend on `networking` or `storage` directly (they operate on already-fetched bytes/values — this is what keeps them unit-testable without IO; see AGENTS.md §3 Dependency Direction below).
- Platform (Win32) code that isn't browser-UI-window-lifecycle (which GPUI owns) → `platform-windows`.
- Generated/build artifacts (`target/`, `Cargo.lock` is source-controlled but never hand-edited) are never manually modified — regenerate via `cargo build`/`cargo update`, don't patch them directly.
- **Before creating a new file, search for an existing home first** (see AGENTS.md §8). A new top-level crate is a significant structural decision — don't add one without confirming it doesn't fit into an existing crate's stated responsibility above, and flag it explicitly if you believe a new crate is genuinely warranted rather than silently creating one.

**Dependency direction (enforced, not aspirational):**
`soul-shell` → `soul-backend-gpui` → `soul-ui`/`soul-core` → `ipc` → {`html`,`css`,`dom`,`layout`,`javascript`,`networking`,`storage`,`compositor`} → {`gpu`,`text-shaping`,`raster`,`image-decode`,`media`,`platform-windows`} → `common`. No cycles. `gpui` appears in exactly one `Cargo.toml` in the whole workspace (`soul-backend-gpui`'s). Verify with:

```sh
cargo metadata --format-version 1 | jq '.packages[] | select(.name != "soul-backend-gpui") | select(.dependencies[].name == "gpui")'
```
This should return nothing. If it doesn't, that's a broken build, not a style nit — fix it before anything else.

---

## 4. Architecture Summary

Full detail lives in `docs/architecture-plan.md` §4–§23; ADRs in §30. The load-bearing points an agent needs at all times:

- **Ownership axis:** GPUI/`soul-backend-gpui` owns the *browser UI* (windows, tabs, omnibox, menus) exclusively. The engine (`html`→`dom`→`css`→`layout`→`paint`→`compositor`) owns *page content* exclusively. Never let one draw or parse the other's domain.
- **Process axis:** currently single-process, multi-threaded (§6, §7). Every cross-boundary call is still a typed command/event enum sent over a channel (`ipc` crate), not a direct function call, even though no OS process boundary exists yet. This is not optional ceremony — it's what avoids the M14–M16 rewrite.
- **GPUI boundary:** `SoulBackend` trait in `soul-ui`. GPUI never receives DOM/layout/CSS data — only a composited texture and routed input events. The compositor never draws browser UI.
- **JS engine:** `boa` (pure Rust) is the default per ADR-4, unless Spike 0(b) triggered a documented pivot to `rquickjs`. Check which is actually wired into `javascript` before assuming.
- **Threading:** UI thread (GPUI, non-blocking) · soul-core thread · renderer thread(s) (one per active tab/window) · compositor thread (independent of renderer — this is what keeps scrolling smooth) · tokio network runtime · IO/disk thread pool. Don't block the UI or compositor thread on network/disk/layout work — route through the appropriate channel instead.
- **Storage:** SQLite (`rusqlite`, WAL mode) for cookies/history/bookmarks/LocalStorage; blob files + SQLite index for HTTP cache/Cache Storage; SessionStorage is in-memory only, never persisted (this is a correctness requirement, not an optimization — don't "fix" it by persisting it).

---

## 5. Implementation Rules

- **Error handling:** library crates (everything except `soul-shell`) define their own error enum via `thiserror`, never `anyhow`. `anyhow` is permitted only in `soul-shell`'s top-level glue where errors are terminal. Don't `unwrap()`/`expect()` on anything derived from network, disk, or parsed-content data — those are the untrusted/fallible boundary (`docs/architecture-plan.md` §23). `unwrap()` is acceptable only on invariants the type system already guarantees (e.g., a `NodeId` you just inserted).
- **Logging:** `tracing`, not `println!`/`eprintln!`, anywhere outside throwaway spike code. Use `tracing::instrument` on cross-crate entry points (command handlers, navigation state transitions) so a trace can be followed across the message-passing boundaries described above. Log at the boundary where a decision is made, not at every intermediate call.
- **Async/threading:** `tokio` is confined to `networking`, `storage`, and IO-bound work — never introduce `tokio` into `dom`/`css`/`layout`/`paint`, which are synchronous by design (§4 ownership axis; these run on the renderer thread, not an async runtime). Cross-thread communication is `tokio::sync::mpsc` (async boundaries) or `crossbeam-channel` (sync, latency-sensitive boundaries like renderer→compositor) — match the existing pattern for the boundary you're touching rather than picking whichever you personally reach for.
- **State management:** DOM state lives in the arena-based `NodeId` system in `dom` — never introduce a parallel `Rc<RefCell<_>>` tree alongside it. Mutation goes through the one mutation API that also records invalidation (§13) — don't add a second mutation path that bypasses dirty-bit tracking, even for a "simple" internal case.
- **API boundaries / message shape:** cross-crate commands are enums, not trait objects with many methods, per ADR-5. When adding a new cross-crate capability, ask "what message would this be once it's cross-process" and design the in-process version to match that shape now.
- **File/module size:** files must target **~300–400 lines maximum**. If a file is pushing past ~300–400 lines, split it by responsibility into cohesive submodules/files (e.g. `module/submodule.rs`). Never leave bloated multi-concern files.
- **Test file structure:** crate-level integration tests live under `crates/<crate>/tests/<topic>_tests.rs` (following Rust integration test naming conventions), keeping `src/` clean. Inline `#[cfg(test)] mod tests` is reserved for small private unit tests only.
- **Introducing abstractions:** don't add a trait/generic abstraction until there are two concrete call sites that need it, or the plan document explicitly calls for one (e.g., `SoulBackend`, the `raster`/GPU-raster swappable-backend trait described in `docs/architecture-plan.md` §17). A single call site doesn't need an abstraction — write the concrete thing.
- **Naming:** crates and files are lowercase kebab-case (already reflected in AGENTS.md §3's layout) — Rust module/type/fn naming otherwise follows standard `rustfmt`/API-guideline conventions; don't invent a project-specific style.

---

## 6. Dependency Policy

Full per-subsystem table: `docs/architecture-plan.md` §25. The policy, condensed:

- **Reuse, don't reinvent:** TLS/crypto (`rustls`), DNS (`hickory-resolver`), HTTP (`hyper`/`h2`/`h3`+`quinn`), HTML tokenizing (`html5ever`), CSS tokenizing/matching primitives (`cssparser`/`selectors`), text shaping (`cosmic-text`), image decode (`image`), SVG (`resvg`), 2D raster (`tiny-skia`), GPU abstraction (`wgpu`), JS engine (`boa_engine`), storage (`rusqlite`).
- **Build from scratch (the actual differentiator):** CSS cascade/computed-style resolution, block/inline layout, DOM/JS binding design, the event loop, the tab-tiering lifecycle, GPUI↔compositor integration.
- **A new dependency is allowed when:** it solves a genuinely solved problem this project shouldn't re-solve (matches the "reuse" category above), and it doesn't duplicate a crate already in the workspace doing the same job.
- **A new dependency is NOT allowed merely because it makes one function easier to write.** If you're reaching for a crate to avoid writing ~30–50 lines of straightforward logic in a crate that's supposed to own that logic from scratch (layout, cascade, DOM/JS bindings), write the logic instead.
- **Prohibited outright:** anything that reintroduces Electron/WebView2/a complete embedded browser engine; a second general-purpose language runtime (Node.js, .NET); a crate that duplicates an already-chosen crate's job (e.g., a second HTTP client library, a second SQLite wrapper) without an ADR explaining the replacement.
- **Security-critical primitives** (crypto, TLS, parsing of untrusted bytes where a mature hardened crate exists) are never hand-rolled without a specific, reviewed ADR — this is a hard rule, not a case-by-case judgment call.
- **Before adding any dependency:** check `docs/architecture-plan.md` §25's table first — it may already have a stated answer for exactly this subsystem. If it doesn't, and the addition is nontrivial (not a tiny utility crate like `unicode-linebreak`), note it as a candidate ADR rather than silently adding it.

---

## 7. Agent Workflow

**Understand → Inspect → Plan → Implement → Integrate → Validate → Review → Finish.** Do not skip from "Understand" to "Implement."

1. **Understand:** restate the request as a concrete requirement. If it's vague ("add tab freezing"), figure out which milestone/subsystem it actually belongs to (AGENTS.md §9 Task Decomposition) before touching code.
2. **Inspect:** follow AGENTS.md §8 below — read this file, the relevant part of `docs/architecture-plan.md`, and the actual current code (not just what the plan says *should* exist — verify what's really there).
3. **Plan:** identify affected crates, the message/API shape needed at each boundary, what's genuinely new vs. what already exists to extend, and what the Definition of Done (AGENTS.md §13) looks like for this specific task before writing code.
4. **Implement:** smallest coherent change that satisfies the plan (AGENTS.md §11). Follow AGENTS.md §5's implementation rules.
5. **Integrate:** wire it into every real caller/consumer — see AGENTS.md §12. A function that compiles but isn't called from anywhere real is not done.
6. **Validate:** run the checks in AGENTS.md §14. Don't claim a check passed without running it.
7. **Review:** self-review the diff per AGENTS.md §18 before reporting completion.
8. **Finish:** report per AGENTS.md §26 — what was inspected, what changed, what was validated, what's still open.

---

## 8. Repository Inspection Procedure

Before writing any code:

1. Read this file in full if you haven't already this session.
2. Read the relevant numbered section(s) of `docs/architecture-plan.md` for the subsystem you're touching — not the whole document every time, but don't skip the specific section either.
3. `view` the target crate's directory tree and its `lib.rs`/`mod.rs` to see what already exists.
4. Search for existing types/functions that might already do (or partially do) what's being asked — grep for the concept, not just an exact expected function name (e.g., searching for "invalidat" when adding style-invalidation logic, not just `invalidate_style`).
5. Check `Cargo.toml` for the crate to see what's already a dependency — don't assume you need to add one before checking.
6. Check `tests/` and any in-crate `#[cfg(test)]` modules for existing coverage of the area — tests document actual intended behavior better than comments do.
7. Check recent git history/log for the touched files if available — recent commits often explain *why* something looks the way it does, which prevents "fixing" an intentional decision.
8. Explicitly decide and state: does the requested functionality already partially exist? If yes, extend it; don't create a parallel implementation (AGENTS.md §10).

---

## 9. Task Decomposition

Vague request → before coding, write out (even briefly, in your own reasoning or a short plan comment):

- **Requirements:** what does "done" actually mean here, in concrete terms?
- **Constraints:** which of AGENTS.md §2's non-negotiables and AGENTS.md §5's implementation rules apply?
- **Affected subsystems:** which crate(s), per AGENTS.md §3/§4?
- **Implementation steps:** in the order they need to happen (respecting the dependency direction in AGENTS.md §3).
- **Integration points:** every caller, config path, UI exposure, persistence path, and error path this touches (AGENTS.md §12).
- **Validation criteria:** what specifically will you run/check to confirm it works (AGENTS.md §14)?

Don't jump straight into editing a file because the request "sounds simple." Most browser-engine tasks that sound simple ("just add X to the DOM API") have integration surface (JS bindings, invalidation, tests) that isn't visible from the request text alone.

---

## 10. "Do Not Reinvent Existing Functionality"

Before creating any of the following, search first — a duplicate is worse than a slightly-imperfect reuse of the existing one, because it creates two sources of truth:

- Utilities/helpers (check `common` first)
- Error types (check whether the target crate already has a `thiserror` enum to extend)
- Configuration/state containers (check `soul-core` for existing state-machine patterns before adding a new one)
- Platform wrappers (check `platform-windows` before writing new raw `windows`-crate FFI)
- Rendering/raster utilities (check `raster`/`paint`/`compositor` before adding drawing primitives elsewhere)
- Logging setup (check `common`'s `tracing` init — don't re-initialize per crate)
- Resource-management/lifecycle code (check whether `soul-core`'s tab-tiering lifecycle already covers the resource-cleanup case you're solving)

If you're confident something doesn't exist yet after actually searching (AGENTS.md §8), say so explicitly in your plan rather than silently assuming.

---

## 11. Change-Scope Discipline

- Make the **smallest coherent change** that satisfies the task's Definition of Done (AGENTS.md §13).
- Do not rewrite unrelated code, rename unrelated files/types, or reorganize a crate "while you're in there."
- Do not migrate architecture (e.g., swapping a channel type, changing the dependency-direction rule, altering the `SoulBackend` trait shape) without an explicit requirement to do so — these are ADR-level decisions (§30 of the plan), not incidental to a feature task.
- Do not perform speculative "cleanup" of unrelated code during feature work, even if you notice something that looks wrong — note it instead (in your final report, per AGENTS.md §26) rather than fixing it unprompted.
- Do not create speculative abstractions "in case it's needed later" — see AGENTS.md §5.
- **A larger refactor is genuinely justified when:** the current structure actively prevents implementing the requested feature correctly (not just "makes it slightly more verbose"), or the plan document itself calls for the refactor at this milestone (e.g., M14's IPC-transport work is *supposed* to touch many call sites — that's in scope for that specific milestone, not scope creep). If you believe a refactor is justified, state the justification explicitly before doing it rather than doing it silently.

---

## 12. Integration-First Thinking

A feature is not complete because one function/module works in isolation. Before reporting completion, verify:

- **Callers** — is it actually invoked from a real path (navigation flow, JS binding, UI action), not just a unit test?
- **Initialization/lifecycle** — does it need wiring into tab creation/teardown, window lifecycle, or profile setup?
- **Configuration** — does it need a settings/config surface, even a minimal one?
- **UI/API exposure** — does the user or JS-facing surface actually reach this code?
- **Persistence** — does state need to survive a restart (session restore, storage)? Don't forget SQLite schema/migration if so.
- **Error paths** — what happens on failure, not just the happy path?
- **Shutdown paths** — is cleanup handled on tab close / app quit, not just creation?
- **Platform integration** — does this need a `platform-windows` hook (clipboard, file dialog, notifications, etc.)?
- **Tests** — per AGENTS.md §24.
- **Build/release integration** — does this affect `soul-shell`'s wiring, CI, or (post-M23) packaging?

---

## 13. Definition of Done

A task is complete only when **all** of the following are true:

- [ ] Implementation exists and matches the plan/requirements.
- [ ] Integration is complete per AGENTS.md §12 above — not just the isolated module.
- [ ] Old/superseded code paths are removed or clearly deprecated, not left as silent dead code.
- [ ] Error paths are handled, not just the happy path.
- [ ] Relevant tests exist and pass (AGENTS.md §14/§24) — new logic without any test coverage is not done, absent a stated reason (e.g., genuinely requires manual GPU/hardware verification).
- [ ] `cargo fmt --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- [ ] `cargo build --workspace` (and `--release` if the task is performance-relevant) succeeds.
- [ ] Platform-specific behavior (Win32/DPI/GPU) is verified where relevant, or explicitly flagged as unverified if it can't be (e.g., no GPU available in the current environment).
- [ ] Documentation updated where AGENTS.md §19 requires it.
- [ ] No accidental files, debug prints, commented-out code, or stray `TODO`s without a tracked reason remain.
- [ ] No known regression introduced — re-run the affected crate's existing test suite, not just new tests.
- [ ] The dependency-direction check (AGENTS.md §3) still passes if any crate boundary was touched.

If any box can't be checked, the task is not done — say so plainly rather than reporting completion.

---

## 14. Validation Strategy

```sh
cargo fmt --all --check                                   # mandatory, every change
cargo clippy --workspace --all-targets -- -D warnings       # mandatory, every change
cargo test --workspace                                      # mandatory, every change touching logic
cargo build --workspace                                     # mandatory
cargo build --workspace --release                            # mandatory if perf-relevant (layout/paint/compositor)
cargo metadata --format-version 1 | jq '...'                # mandatory if any crate's Cargo.toml changed (AGENTS.md §3 check)
cargo bench -p layout / -p paint / -p parse                  # conditional — only if touching a benchmarked subsystem, compare against baseline in benchmarks/
cargo test -p <crate> --test wpt_subset                      # conditional — only once M20-era WPT harness exists
```

- **Mandatory on every change:** format, clippy, unit/integration tests for the touched crate(s), workspace build.
- **Conditional:** release build (perf-sensitive changes), benchmarks (layout/paint/compositor changes), screenshot/golden-image tests (paint/compositor changes, once that harness exists per `docs/architecture-plan.md` §27), fuzz targets (only when directly touching a parser/deserializer boundary — don't run a full fuzz campaign for an unrelated change, but do check existing fuzz corpus tests still pass if you touched `html`, `css`, or `ipc` deserialization).
- **GPU/platform-specific validation:** if the environment has no real GPU/Windows target available, say so explicitly rather than claiming it was verified — this is a real limitation of many CI/agent environments and should be reported, not hidden.

---

## 15. Debugging Methodology

- Reproduce the failure before changing code, when a reproduction is feasible.
- Identify the actual failure boundary (which crate, which message boundary, which thread) before guessing at a fix — use `tracing` output and the crate boundaries in AGENTS.md §3/§4 to narrow it down.
- Trace the data/control flow through the message-shaped APIs (§4) rather than assuming a direct call path — remember most cross-crate interaction goes through a channel, not a function call.
- Inspect logs/errors from the actual failing layer, not just the top-level symptom.
- Make one logical change at a time and re-validate (AGENTS.md §14) before making the next — don't batch multiple speculative fixes together.
- Avoid random trial-and-error edits; if you don't have a hypothesis for *why* a change would fix the bug, form one first (even from limited evidence) rather than editing to see what happens.

---

## 16. Common Failure Modes (explicit warnings)

Do not:
- Implement only the visible UI/API surface without wiring the backend (`soul-core`/relevant engine crate) — see AGENTS.md §12.
- Add a duplicate system where one already exists (AGENTS.md §10).
- Bypass an existing abstraction (e.g., mutating DOM state outside the mutation API, or calling `gpui` directly instead of going through `SoulBackend`) because it's momentarily more convenient.
- Silently change existing behavior as a side effect of an unrelated fix.
- Ignore Windows-specific behavior (DPI, WorkerW-equivalent concerns, Media Foundation quirks) in favor of a "generic" implementation that happens to compile.
- Leave dead code behind after a change makes it unreachable.
- Forget storage schema migration when changing a persisted shape (AGENTS.md §19/§12).
- Forget shutdown/cleanup paths (AGENTS.md §12).
- Assume a feature is complete because it compiles — compilation success is a precondition for done, not evidence of done.
- Add a dependency because it's easier (AGENTS.md §6).
- Perform a broad refactor for a small, targeted task (AGENTS.md §11).
- Create a new file/crate in the wrong location instead of extending the crate that already owns that responsibility (AGENTS.md §3/§10).
- Manually hand-edit a generated/build artifact.
- Ignore CI or (post-M23) release-packaging implications of a change.

---

## 17. Decision-Making Hierarchy

When multiple approaches seem plausible, prioritize in this order:

**Existing project architecture (AGENTS.md §3/§4, `docs/architecture-plan.md`) > explicit requirements of the current task > established conventions already visible in the codebase > the minimal/simplest correct implementation > personal/stylistic preference.**

When uncertain:
- Inspect more code before guessing (AGENTS.md §8).
- Search for precedent elsewhere in the codebase for how a similar problem was already solved.
- Do not invent new behavior not implied by the requirement or the plan — ask or flag instead of assuming.
- Preserve existing behavior unless the task specifically requires changing it.
- Document any assumption you had to make, explicitly, in your final report (AGENTS.md §26).

---

## 18. Change Verification (self-review before reporting done)

Before finishing, review your own diff and answer honestly:

- What files changed, and why did *each* one need to change?
- Is every change necessary for this task, or did something unrelated sneak in?
- Did the implementation bypass an existing abstraction or architectural boundary (AGENTS.md §3/§16)?
- Did any unrelated behavior change as a side effect?
- Are there unused imports, dead code, leftover debug logging, temp files, or undocumented `TODO`s?
- Does the final state of the repository actually satisfy the original request — not just "does it compile," but "does AGENTS.md §13's checklist fully pass"?

---

## 19. Documentation Rules

Update, when the change warrants it:
- **This file (`AGENTS.md`)** — if the change alters a rule, boundary, or workflow described here (e.g., a new crate added to AGENTS.md §3, a new mandatory validation step).
- **`docs/architecture-plan.md`** — if the change is a genuine architecture decision (new ADR, milestone reordering, scope change to a phase). Follow the plan's own convention: ADRs are the source of truth over diagrams (see the plan's closing note) — update the ADR, then update any diagram that now disagrees with it.
- **README** (once one exists) — user/developer-facing setup or usage changes.
- **Per-crate doc comments** (`//!` module docs) — when a crate's responsibility or public API shape changes.
- **Migration notes** — any time a persisted schema (SQLite, session-restore format) changes shape.
- **Changelog/release notes** — once the project has a release cadence (post-M23-adjacent); not needed pre-MVP.

Don't update documentation reflexively for every change — a purely internal refactor with no behavioral or architectural change doesn't need a plan-document update, just accurate code comments if the "why" isn't obvious from the diff.

---

## 20. Git Discipline

- Never modify `.git` internals directly.
- Never rewrite history (`rebase -i`, `force-push`) unless explicitly requested.
- Never discard the user's uncommitted changes.
- Never reset or revert work unrelated to the current task.
- Preserve existing uncommitted changes in the working tree — inspect before assuming a clean state.
- Keep commits/changes focused on the stated task (AGENTS.md §11).

---

## 21. Security and Safety

- Never expose secrets (API keys, tokens, credentials) in code, logs, or commit messages.
- Never hardcode credentials — this project's own review history already flagged and fixed exactly this class of bug once (a hardcoded revalidation secret in a prior related project); treat it as a standing lesson, not a one-off.
- Never weaken a security control (SOP/CORS enforcement, TLS validation, IPC message validation, the dangerous-URL-scheme allowlist) just to make a feature work or a test pass. If a security check is blocking a feature, that's a design conversation, not a "temporarily disable it" moment.
- Treat all network responses, parsed HTML/CSS/JS content, and IPC messages as **untrusted input** (`docs/architecture-plan.md` §23/§8) — validate before use, don't `unwrap()` on it (AGENTS.md §5).
- Respect platform security boundaries — don't broaden `file://` access, don't add a `ShellExecute` path outside the existing allowlist, without it being the explicit point of the task.

---

## 22. Performance Philosophy

- Avoid premature optimization — correctness first, especially in early milestones (M0–M13) where the pipeline isn't proven yet.
- Avoid unnecessary allocations/copies in hot paths (layout, paint, compositor) specifically — these are the paths where it matters; don't apply the same scrutiny to cold paths (settings UI, one-time startup code) at the cost of readability.
- Consider CPU, memory, GPU, I/O, startup time, and resource lifetime for anything touching the renderer/compositor/layout crates.
- Do not sacrifice correctness for speculative performance — an incorrect-but-fast layout result is worse than a correct-but-unoptimized one at this stage of the project.
- Profile before making a non-obvious optimization decision when practical (`cargo bench`, existing `benchmarks/` baselines) — don't guess at what's slow.

---

## 23. Platform-Specific Rules

- **Portable/core logic** (`dom`, `css`, `layout`, `paint`, `javascript`) contains zero Win32/platform-specific code.
- **Platform abstraction** — `SoulBackend` (UI), and the swappable raster-backend trait (`docs/architecture-plan.md` §17) are the seams where platform-specific implementations plug in.
- **Platform implementation** — `soul-backend-gpui` (UI), `platform-windows` (Win32 wrappers), `gpu` (wgpu/DXGI specifics), `media` (Media Foundation) are where OS/hardware-specific code is *allowed* to live.
- Do not leak Win32 types, DirectWrite handles, or DXGI specifics into `dom`/`css`/`layout`/`javascript` — if a portable crate needs a platform capability, it goes through a trait defined in the portable crate and implemented in the platform crate, not a direct dependency in the other direction.

---

## 24. Testing Philosophy

- **Unit tests:** per-function/per-module, colocated in the crate strictly for small private logic.
- **Component/integration tests:** placed in `crates/<crate>/tests/<name>_tests.rs` adhering to Rust test naming best practices; cross-crate end-to-end tests live under workspace `tests/`.
- **Manual testing:** GPU/visual correctness where no screenshot-test harness exists yet, real-hardware DPI/multi-monitor behavior — required when automated coverage isn't feasible, but must be stated explicitly as manual, not silently skipped.
- **Target-platform-required tests:** anything touching Media Foundation, DirectWrite, DXGI, or Job Objects/sandboxing needs verification on real Windows, not just "compiles" — flag if the current environment can't provide this.
- **Regression tests:** every bug fix gets a fixture/test that reproduces the original failure, added permanently at the appropriate tier (`docs/architecture-plan.md` §27 has the full hierarchy) — don't fix a bug without adding the regression test in the same change.

---

## 25. Feature Lifecycle

**Requirement → Design → Implementation → Integration → Testing → Documentation → Packaging → Verification.**

Skipping Integration or Testing does not count as reaching "done" — see AGENTS.md §12's Integration-First Thinking and §13's Definition of Done. A feature that's implemented but not integrated, or implemented but not tested, is a work-in-progress, and should be reported as such rather than as complete.

---

## 26. Agent Communication

When reporting back:
- State what you actually inspected (files/sections read), not a generic summary.
- State what you actually changed, file by file if the change set is small, or by subsystem if large.
- Call out important decisions made along the way (e.g., "extended the existing X rather than creating a new type because Y").
- State what validation was actually performed (AGENTS.md §14) — and just as importantly, what validation was **not** performed and why (e.g., no GPU available in this environment).
- State remaining limitations or follow-up work explicitly.
- **Never claim something was tested when it wasn't.** "This should work" is not the same as "I ran the tests and they passed" — say which one is true.

---

## 27. Final Self-Review Checklist

Run through this before reporting a task complete:

- [ ] I read this file and the relevant section(s) of `docs/architecture-plan.md` for this task.
- [ ] I inspected the actual current repository state rather than assuming it matches the plan document.
- [ ] I searched for existing implementations before writing new code (AGENTS.md §10).
- [ ] My change is the smallest coherent solution to the stated problem (AGENTS.md §11).
- [ ] I did not cross a crate-boundary rule (GPUI isolation, dependency direction, portable-vs-platform separation) without an explicit reason.
- [ ] I wired the change into every real caller/config/persistence/error/shutdown path it needs (AGENTS.md §12).
- [ ] I ran `cargo fmt`, `cargo clippy`, `cargo test`, and `cargo build` (and release/bench/metadata checks where applicable) and know their actual results.
- [ ] I reviewed my own diff for unrelated changes, dead code, and debug artifacts (AGENTS.md §18).
- [ ] I updated documentation where warranted (AGENTS.md §19), and did not update it reflexively where not.
- [ ] My final report is honest about what was and wasn't validated (AGENTS.md §26/§13 above), and about any assumptions made.