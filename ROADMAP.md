# ROADMAP.md

> **Project:** Lumo File Explorer
> **Strategy:** Independent phases with clear success criteria and mandatory tests

---

## Phase 0: Foundation (Week 1-2)
**Goal:** Project skeleton, cross-platform build infrastructure, basic CLI

### Deliverables
- [x] Cargo workspace with one crate per layer (`lumo-core`, `lumo-cli`; `lumo-platform`, `lumo-ui` added later)
- [x] Minimal CLI that lists directory contents (`lumo ls`)
- [x] Native build targets x86_64-pc-windows-msvc and x86_64-unknown-linux-gnu (built by CI)
- [x] CI pipeline (GitHub Actions) running fmt, clippy, tests on Windows + Linux
- [x] `README.md` with build instructions
- [x] Toolchain pinned in `rust-toolchain.toml`; MIT license

### Dependencies
None

### Mandatory Tests
- [x] `cargo build --release` succeeds on Windows (Linux: verified by CI once pushed)
- [x] `cargo test` passes all unit tests
- [x] Binary executes and outputs version number
- [ ] CI matrix passes (windows-2022, windows-2025, ubuntu-22.04) — needs a GitHub remote. GitHub no longer offers Windows 10 runners, so Windows 10 is tested manually.

### Success Criteria
```bash
cargo run --bin lumo -- ls /some/path
# Outputs one entry per line (directories first, with trailing separator) without panic
```

---

## Phase 1: Core Filesystem Engine (Week 3-5)
**Goal:** High-performance directory scanning with streaming support

### Deliverables
- [ ] Async directory traversal using `tokio`
- [ ] Streaming iterator for entries (memory-efficient)
- [ ] Platform-specific FS abstractions (`FileSystemProvider` trait)
- [ ] Parallel file metadata fetching using `rayon`
- [ ] Error handling with typed errors (`FsError` enum)

### Dependencies
Phase 0

### Mandatory Tests
- [ ] Unit test: Scan directory with 10K files in <2s (Windows), <1s (Linux)
- [ ] Unit test: Stream 1M entries without memory >500MB
- [ ] Integration test: Handle permission-denied paths gracefully
- [ ] Benchmark: Compare scan speed against standard `std::fs::read_dir`

### Success Criteria
```rust
let mut stream = fs.scan_async("/large/directory").await?;
while let Some(entry) = stream.next().await {
    // Process without blocking
}
// Memory stays <100MB throughout
```

---

## Phase 2: Basic UI Layer (Week 6-8)
**Goal:** Functional windowed interface with tabs and navigation

### Deliverables
- [ ] egui (eframe) UI, shared by all platforms
- [ ] Tabbed browsing (multiple folders)
- [ ] Address bar with path history
- [ ] Back/Forward navigation stack
- [ ] Sidebar with quick access folders
- [ ] Keyboard shortcuts (Ctrl+W, Alt+Left, Ctrl+Tab)
- [ ] `lumo [PATH]` opens a window at PATH (needed by Phase 3 shell hooks)

### Dependencies
Phase 1

### Mandatory Tests
- [ ] Open 50 tabs without memory leak (<500MB total)
- [ ] Tab switch latency <50ms
- [ ] Address bar autocomplete works
- [ ] Window resize maintains 60fps rendering
- [ ] Crash recovery restores tabs after force-close

### Success Criteria
```
User opens app → navigates to 5 folders → tabs persist after close → reopen → tabs restored
```

---

## Phase 3: Windows Shell Integration (Week 9-10)
**Goal:** Intercept Win+E and system folder launches

### Deliverables
- [ ] Win+E override (HKCU per-user handlers; exact keys to be verified on Windows 10 and 11)
- [ ] Folder type association override (Downloads, Documents, etc.)
- [ ] Context menu extension registration
- [ ] First-run configuration wizard (one-time setup)
- [ ] Rollback mechanism to restore default Explorer

### Dependencies
Phase 2

### Mandatory Tests
- [ ] Win+E opens Lumo instead of Explorer
- [ ] Clicking Downloads in browser opens Lumo
- [ ] Right-click context menu shows "Open with Lumo"
- [ ] Uninstall process removes all registry changes
- [ ] Test on Windows 10 and 11 separately

### Success Criteria
```powershell
# After first-run wizard completes
explorer.exe  # Should NOT launch on Win+E
lumo          # Launches instead
```

---

## Phase 4: Linux Desktop Integration (Deferred — Windows first)
**Goal:** Ship the shared egui app on Linux with native desktop integration

> Deferred until the Windows path (Phases 2–3, 5+) is solid. CI keeps the Linux build green in the meantime.

### Deliverables
- [ ] Linux build of the shared egui UI (X11 + Wayland via winit)
- [ ] Desktop file integration (`lumo.desktop`)
- [ ] MIME type handler for `inode/directory`
- [ ] GNOME/KDE/XFCE configuration scripts

### Dependencies
Phase 2

### Mandatory Tests
- [ ] `xdg-mime query default inode/directory` returns `lumo.desktop`
- [ ] Double-clicking folder in Nautilus/Dolphn opens Lumo
- [ ] Works under GNOME (X11 + Wayland), KDE Plasma, XFCE
- [ ] Tab persistence works identically to Windows

### Success Criteria
```bash
# Set default file manager
xdg-mime default lumo.desktop inode/directory

# Test
nautilus ~/Documents  # Opens Lumo instead of Nautilus
```

---

## Phase 5: Search & Indexing (Week 14-16)
**Goal:** Fast file search with optional background indexing

### Deliverables
- [ ] Real-time filename search (ripgrep integration)
- [ ] Content search with match highlighting
- [ ] Indexed search mode (SQLite + vector embeddings optional)
- [ ] Saved searches and filters (date, size, type)
- [ ] Semantic search stub (interface ready for AI integration)

### Dependencies
Phase 2

### Mandatory Tests
- [ ] Search 10K files in <100ms (indexed), <500ms (real-time)
- [ ] Regex search handles complex patterns
- [ ] Content search finds text within PDFs, DOCX, TXT
- [ ] Index rebuild takes <5min for 100K files

### Success Criteria
```bash
lumo search "TODO" --path ./src --type "*.rs"
# Returns results in <500ms
```

---

## Phase 6: Developer Tools (Week 17-19)
**Goal:** Integrated terminal, Git status, and code previews

### Deliverables
- [ ] Inline terminal (WSL on Windows, bash/zsh on Linux)
- [ ] Git status overlays (modified/untracked/staged indicators)
- [ ] Diff viewer for selected files
- [ ] Syntax-highlighted code previews
- [ ] File watcher hooks (trigger on change)

### Dependencies
Phase 5

### Mandatory Tests
- [ ] Open terminal in current folder in <100ms
- [ ] Git status updates within 500ms of file change
- [ ] Diff viewer loads 10MB files without freeze
- [ ] Syntax preview for 20+ programming languages

### Success Criteria
```
Select .rs file → Press Space → Preview appears with syntax highlight
Modify .rs file in editor → Git tab shows "Modified" in red
```

---

## Phase 7: Preview System (Week 20-22)
**Goal:** Comprehensive file preview panel with extension framework

### Deliverables
- [ ] Built-in image viewer (PNG, JPEG, WEBP, SVG)
- [ ] Text/Markdown/code preview with wrapping
- [ ] PDF viewer (embedded or plugin)
- [ ] Media playback (MP4, MP3, AVI)
- [ ] Thumbnail cache with disk persistence
- [ ] Internal `PreviewHandler` trait (third-party preview plugins arrive with the Phase 12 SDK)

### Dependencies
Phase 6

### Mandatory Tests
- [ ] Preview 100MB image in <500ms
- [ ] Render 50-page PDF in <2s
- [ ] Video playback smooth at 60fps
- [ ] Thumbnail generation uses <100MB RAM
- [ ] Unknown formats fall back to a hex/text preview without error

### Success Criteria
```
Hover over image → Thumbnail generates
Click image → Full preview opens instantly
```

---

## Phase 8: Batch Operations (Week 23-25)
**Goal:** Advanced file manipulation with undo capability

### Deliverables
- [ ] Bulk rename with regex, templates, AI suggestions
- [ ] Duplicate finder (checksum + fuzzy matching)
- [ ] Job queue with undo/redo support
- [ ] Copy/move progress with pause/resume
- [ ] Folder synchronization tool

### Dependencies
Phase 7

### Mandatory Tests
- [ ] Rename 10K files with regex in <5s
- [ ] Find duplicates in 100GB dataset in <10min
- [ ] Undo batch rename restores original names
- [ ] Pause copy at 50%, resume without corruption
- [ ] Sync two folders with 5K differences in <2min

### Success Criteria
```
Select 1000 photos → "Rename with date" → All renamed correctly
Press Ctrl+Z → All names reverted instantly
```

---

## Phase 9: Local AI Integration (Week 26-30)
**Goal:** Offline AI assistant for file analysis and organization

### Deliverables
- [ ] Ollama integration for local LLMs
- [ ] Semantic search with vector embeddings
- [ ] AI-powered file renaming suggestions
- [ ] Auto-organization recommendations
- [ ] Chat panel with file context
- [ ] MCP (Model Context Protocol) server for agents

### Dependencies
Phase 7

### Mandatory Tests
- [ ] AI rename suggestion for 100 files in <10s (offline)
- [ ] Semantic search "invoice from last week" returns correct file
- [ ] Chat with PDFs extracts answers accurately
- [ ] No network calls when offline (verified by firewall)
- [ ] Model download <500MB, RAM usage <2GB during inference

### Success Criteria
```
Type "Find contracts signed in 2025" → Returns relevant PDFs
AI suggests: "Rename IMG_001.jpg to 'Vacation_Hawaii_2025_001.jpg'"
Confirm → Applied instantly
```

---

## Phase 10: Polish & Optimization (Week 31-34)
**Goal:** Performance tuning, theme system, bug fixes

### Deliverables
- [ ] Theme engine (light/dark/custom colors)
- [ ] Keyboard shortcut customization
- [ ] Memory profiling and optimization passes
- [ ] Startup time reduction to <500ms
- [ ] 1000+ issue backlog cleanup
- [ ] Documentation completion (user + dev guides)

### Dependencies
Phase 9

### Mandatory Tests
- [ ] Cold start <500ms on SSD (measured 10x average)
- [ ] 10K file load <800ms
- [ ] Memory usage <50MB idle, <200MB active
- [ ] All automated tests pass with 90%+ line / 80%+ branch coverage
- [ ] Accessibility audit passes (screen reader support)

### Success Criteria
```bash
time ./lumo
# Real user 0.42s
```

---

## Phase 11: Cloud & Remote (Week 35-38)
**Goal:** Network mounts and cloud storage integration

### Deliverables
- [ ] SFTP/SSH mount as local volume
- [ ] S3/GCS/Azure blob mount
- [ ] Google Drive/OneDrive/Dropbox sync
- [ ] WebDAV client
- [ ] Docker container file access

### Dependencies
Phase 10

### Mandatory Tests
- [ ] Mount remote folder with 10K files in <5s
- [ ] Resume interrupted transfer at any point
- [ ] Cache remote files locally with auto-eviction
- [ ] Conflict resolution for simultaneous edits
- [ ] Test with slow networks (3G speeds)

### Success Criteria
```
Add S3 bucket → Appears in sidebar like local drive
Browse 10K objects → Streams in chunks, no freeze
Edit remote file → Saves with conflict warning if changed elsewhere
```

---

## Phase 12: Plugin Ecosystem (Week 39+)
**Goal:** Extensible architecture for third-party contributions

### Deliverables
- [ ] Plugin SDK (Rust + WASM targets)
- [ ] Plugin marketplace (hosted or local)
- [ ] Sandboxed plugin execution
- [ ] Hot-reload plugin updates
- [ ] Community contribution guidelines

### Dependencies
Phase 11

### Mandatory Tests
- [ ] Write and load custom plugin in <5min
- [ ] Plugin crash does not affect core app
- [ ] Security audit: plugins cannot access restricted paths
- [ ] 10+ community plugins submitted and verified

### Success Criteria
```rust
// Community plugin example
#[plugin]
pub fn compress_folder(path: &Path) -> Result<()> {
    // Custom logic isolated from core
}
```

---

## Phase Dependencies Graph

```
Phase 0 → Phase 1 → Phase 2 (UI) → Phase 3 (Win shell)
                        │  └──────→ Phase 4 (Linux, deferred)
                        ↓
                     Phase 5 → Phase 6 → Phase 7 → Phase 8 → Phase 9 → Phase 10 → Phase 11 → Phase 12
```

### Critical Paths
- **Windows (current focus):** 0 → 1 → 2 → 3 → 5 → 6 → 7 → 8 → 9 → 10
- **Linux (deferred):** 0 → 1 → 2 → 4, then the shared feature phases
- **Post-launch:** 10 → 11 → 12

---

## Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| Windows shell integration fails | Blocks entire project | Have manual fallback (run app manually) |
| Linux DE fragmentation | Delays Phase 4 (already deferred) | Test on 3 DEs minimum, drop exotic ones |
| AI model size too large | Hinders adoption | Offer smaller models (7B params max) |
| Memory leaks in long sessions | User trust loss | Continuous profiling, nightly leak tests |
| Plugin security vulnerabilities | Reputation damage | Sandboxing + signature verification |

---

## Success Metrics Per Phase

Each phase must demonstrate **measurable improvement** over previous state:

| Phase | Metric to Track | Baseline | Target |
|-------|-----------------|----------|--------|
| 0 | Build time | N/A | <30s |
| 1 | Scan 10K files | 5s (std::fs) | <2s |
| 2 | Tab switch | N/A | <50ms |
| 3 | Win+E redirect | Explorer launches | Lumo launches |
| 4 | Linux launch | N/A | <1s |
| 5 | Search 10K files | N/A | <100ms |
| 6 | Git status update | N/A | <500ms |
| 7 | Preview 100MB image | N/A | <500ms |
| 8 | Rename 1K files | N/A | <5s |
| 9 | AI rename 100 files | N/A | <10s |
| 10 | Cold start | 1.5s | <500ms |
| 11 | Remote mount | N/A | <5s |
| 12 | Plugin load | N/A | <2s |

---

*Last updated: October 28, 2026*
