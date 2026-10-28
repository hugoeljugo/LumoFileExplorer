# CLAUDE.md

> **Project:** Lumo File Explorer — A Rust-based, cross-platform file manager
> **Purpose:** Guidelines for AI-assisted development to ensure code quality, consistency, and success

---

## Project Context

### Core Architecture
- **Language:** Rust, edition 2024, toolchain pinned in `rust-toolchain.toml` (currently 1.98.1)
- **Cross-Platform:** Windows 10/11 first; Linux (Wayland/X11) deferred to Phase 4
- **UI Toolkit:** egui (eframe) on every platform — one UI codebase; only OS integration differs
- **License:** MIT, open source
- **Async Runtime:** tokio — optional `tokio` feature of `lumo-core`; the core scanner itself is blocking and runtime-free (tokio file I/O is just `spawn_blocking` anyway)
- **Parallel Processing:** rayon — measure before using: on Windows, per-entry metadata is free, so parallelizing it is slower
- **File System Access:** `windows-sys` (Windows), `nix` + `gio` (Linux)

### Design Principles
1. **Performance First** — Sub-500ms cold start, <800ms folder load for 10K files
2. **Privacy by Default** — All AI features optional and local-only
3. **Zero-Configuration** — Sensible defaults, minimal setup required
4. **Developer-First** — Terminal, Git, Docker integrations out of the box
5. **Deterministic Builds** — Reproducible across platforms

---

## Coding Standards

### Code Organization
Cargo workspace, one crate per layer (crates are added when their phase starts):
```
crates/
├── lumo-core/      # Platform-agnostic logic — no OS or UI deps
│   └── src/
│       ├── fs/     # File/Folder abstractions, FsError
│       ├── scan/   # Directory traversal workers (Phase 1)
│       └── index/  # Search indexing (Phase 5)
├── lumo-cli/       # The `lumo` binary (CLI; later also launches the UI)
├── lumo-ui/        # egui app (Phase 2)
├── lumo-platform/  # OS integration (Phase 3+)
│   └── src/{win32,unix}/  # registry/shell hooks; desktop files, MIME
└── lumo-features/  # ai/, git/, terminal/ (split into crates if they grow)
```

### Naming Conventions
| Item | Convention | Example |
|------|------------|---------|
| Crates | kebab-case, `lumo-` prefix | `lumo-core` |
| Modules / files | snake_case | `src/fs/traversal.rs` |
| Public types | PascalCase | `DirectoryEntry`, `FileMetadata` |
| Functions | snake_case | `scan_directory()`, `compute_checksum()` |
| Traits | PascalCase, role noun | `FileSystemProvider`, `ThumbnailProvider` |
| Unit tests | inline `#[cfg(test)] mod tests` | in `fs/mod.rs` |
| Integration tests | `tests/<area>_test.rs` | `tests/ls_test.rs` |

### Enforced Lints (workspace `Cargo.toml`)
`clippy::unwrap_used`, `clippy::expect_used`, `clippy::dbg_macro` and `unsafe_code` are **deny**. Tests are exempt via `clippy.toml`; test helpers that aren't `#[test]` fns need a local `#[allow]`. FFI code that needs `unsafe` gets a scoped `#[allow(unsafe_code)]` with a `// SAFETY:` comment. CI runs `cargo clippy --workspace --all-targets -- -D warnings`.

### Error Handling
```rust
// NEVER unwrap() in production code
let entries = fs::read_dir(path)
    .context("Failed to read directory")?;

// Define typed errors
#[derive(Debug, thiserror::Error)]
pub enum FsError {
    #[error("Permission denied: {0}")]
    PermissionDenied(PathBuf),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Path too long: {0}")]
    PathTooLong(PathBuf),
}
```

### Testing Requirements
- **Unit tests** — 90%+ line / 80%+ branch coverage (GOALS.md QA); core logic should sit well above that
- **Integration tests** — All platform-specific paths tested on CI matrix
- **Performance benchmarks** — Folder scan speed, memory usage tracked
- **CI gates** — No PR merges without passing test suite

---

## Critical Implementation Notes

### Windows Shell Integration
- Win+E / folder-open redirection works through **per-user (HKCU) handler overrides** (`Software\Classes\Folder`/`Directory` open verbs and the Win+E launch CLSID) — *not* `Explorer\Advanced`. Exact keys must be verified on both Windows 10 and 11 in Phase 3.
- Use the Registry API (`windows-sys`), never shell out to `reg.exe`.
- Stay in HKCU so no elevation is needed; record every value written so uninstall/rollback restores the original state exactly.

### Cross-Platform Abstraction
```rust
pub trait FileSystemProvider: Send + Sync {
    fn list_entries(&self, path: &Path) -> Result<Vec<Entry>>;
    fn watch_directory(&self, path: &Path) -> Result<DirectoryWatcher>;
    fn get_thumbnails(&self, files: &[PathBuf]) -> Vec<Thumbnail>;
}

// Implement for Windows, Linux separately
```

### Memory Management
- **Stream directory scans** — Never load 100K entries into memory
- **Lazy thumbnail generation** — Generate on-demand, cache to disk
- **Worker thread pools** — Limit concurrent I/O to prevent saturation

---

## AI Assistant Guidelines

### What to Do
✅ **Always** verify Rust edition compatibility (`rustc --version`)
✅ **Always** check existing code patterns before modifying
✅ **Suggest** tests alongside new features
✅ **Flag** potential race conditions in async code
✅ **Consider** cross-platform implications on every change

### What to Avoid
❌ **Never** propose `unwrap()` without explicit justification
❌ **Never** suggest cloud-based AI without offline fallback
❌ **Never** ignore platform-specific behavior differences
❌ **Never** skip performance benchmarks for hot-path code
❌ **Never** break existing APIs without migration path

---

## Performance Benchmarks (Target Gates)

| Metric | Phase 1 Target | Final Target | Measurement Method |
|--------|----------------|--------------|--------------------|
| Cold Start | <1.5s | <500ms | `hyperfine` on the release binary |
| Folder Load (1K files) | <2s | <200ms | `cargo bench --bench folder_load` (criterion) |
| Search (indexed) | N/A | <50ms | `cargo bench --bench search` (criterion) |
| Memory Footprint | <100MB | <50MB | `heaptrack`, `valgrind` |
| UI Frame Rate | 60fps | 60fps locked | GPU profiler |

---

## Code Review Checklist

Before merging any PR:
- [ ] Tests added and passing
- [ ] Cross-platform build verified (Windows + Linux)
- [ ] Benchmark results compared (no regressions >5%)
- [ ] No `unwrap()` / `expect()` in production paths
- [ ] Error messages actionable and localized
- [ ] Documentation updated (docs/ and doc comments)
- [ ] No debug logging or println! statements left
- [ ] Memory-safe patterns validated (borrow checker friendly)

---

## Security & Privacy

### Hard Constraints
- **No telemetry by default** — Nothing leaves the machine unless the user opts in; crash reports and usage stats are opt-in, off by default, and disclose exactly what they send
- **No network calls** — Unless explicitly user-initiated (cloud mounts) or opted-in reporting
- **AI local-first** — Cloud models require explicit API key opt-in
- **Sandboxing** — Plugin/extension code runs in isolated context

### Audit Points
- Validate all file paths (prevent path traversal attacks)
- Sanitize user input in search queries (prevent regex injection)
- Secure local AI model storage (encrypt model configs)

---

## Getting Help

| Topic | Resource |
|-------|----------|
| Rust async | [`tokio` docs](https://tokio.rs) |
| Windows Shell | [MSDN Namespace Extensions](https://learn.microsoft.com/windows/win32/shell/) |
| GTK4 Rust | [`gtk4-rs` guide](https://gtk-rs.org/gtk4-rs/stable/latest/book/) |
| Performance | [`criterion`](https://bheisler.github.io/criterion.rs/book/) |

---

## Version Control Rules

- **Main branch:** `main` (protected)
- **Feature branches:** `feat/<short-description>`
- **Bugfix branches:** `fix/<issue-number>-<description>`
- **Commits:** Conventional Commits format (`feat:`, `fix:`, `perf:`)
- **PR title:** Must match commit subject line

---

*Last updated: October 28, 2026*
*Generated for: Lumo File Explorer Project*
