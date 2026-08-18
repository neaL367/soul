# Subsystems: Platform, Networking, Storage & Security

This document contains detailed architecture specifications for **Networking, Storage, GPU Architecture, Windows Platform Layer, and Security Architecture** (§19–§23) of the Soul Browser Engine.
For the main architecture index and milestone status, see [`docs/architecture-plan.md`](file:///d:/Hobby/soul/docs/architecture-plan.md).

---

## 19. Networking Stack

```text
URL → url crate (parsing, per WHATWG URL spec)
    → Proxy resolution (system proxy settings via WinHTTP/registry, or manual config)
    → DNS (hickory-resolver, with its own cache; *implementation note 2026-08-18: currently the platform resolver via `tokio::net::lookup_host` — see ADR-19*)
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

**Reuse, do not reinvent:** TLS/crypto (`rustls`, backed by `aws-lc-rs` or `ring` as its crypto provider), DNS resolution, HTTP/1.1/2/3 protocol implementations, QUIC. These are security-critical, extensively fuzzed, and re-implementing them is both a security risk and a waste of the project's differentiation budget. This is a hard rule, not a preference (see §25 dependency policy).

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
- **Encryption**: not in MVP scope; cookie/password encryption-at-rest (DPAPI-backed, matching how Chromium/Edge protect data on Windows) is a Production-Definition requirement (§33), not MVP.

---

## 21. GPU Architecture

### Vulkan vs. D3D12 vs. wgpu (see also ADR-6, §30)

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
- **Sandboxing**: realistic target is Job Objects + restricted access tokens + AppContainer profiles for renderer processes — **not** a claim of Chromium-equivalent sandbox strength (Chromium's Windows sandbox is itself a decade-plus of dedicated engineering). This is stated explicitly in §6/§29 rather than glossed over.

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
