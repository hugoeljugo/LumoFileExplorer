# Lumo File Explorer

A fast, private, developer-friendly file manager written in Rust, targeting
Windows 10/11 first and Linux next.

> **Status:** early development (Phase 2 done). A tabbed file-browsing window
> and the `lumo-cli` tool work; Windows shell integration (Win+E) is next.
> See [ROADMAP.md](ROADMAP.md) for the plan and [GOALS.md](GOALS.md) for targets.

## Building

Requirements: [rustup](https://rustup.rs). The toolchain version is pinned in
`rust-toolchain.toml` and installs automatically on first build.

On Windows you also need the MSVC build tools ("Desktop development with C++"
workload from Visual Studio Build Tools). On Linux, install the windowing
headers: `libxkbcommon-dev libwayland-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libgl1-mesa-dev`.

```sh
cargo build --release          # binaries in target/release/
cargo test --workspace --all-features    # run all tests
cargo test --release -p lumo-ui --test perf_test   # UI timing gates
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo bench -p lumo-core --bench folder_load   # scan benchmarks
```

## Usage

```sh
lumo                           # open the window, restoring the last session's tabs
lumo PATH                      # open the window at PATH (one tab)
lumo-cli --version
lumo-cli ls [PATH]             # list a directory (defaults to the current one)
```

Open tabs are saved on every change to `%APPDATA%\Lumo\data\session.json`
(`~/.local/share/lumo` on Linux), so they survive crashes. Set `LUMO_DATA_DIR`
to store state elsewhere. Lumo follows the system light/dark setting;
`LUMO_THEME=light` or `LUMO_THEME=dark` forces a theme.

### Keyboard shortcuts

| Keys | Action |
|------|--------|
| Ctrl+K or Ctrl+P | Command palette: fuzzy-search commands, tabs, places and files in this folder (`>` for commands only) |
| Right-click, Shift+F10 | Search menu for the clicked item or folder |
| Ctrl+C / Ctrl+X / Ctrl+V | Copy / cut / paste files (works with Explorer) |
| Delete / Shift+Delete | Move to the Recycle Bin / delete permanently (asks first) |
| F2 | Rename in place |
| Ctrl+Shift+N | New folder |
| Ctrl+Z | Undo the last file operation |
| Ctrl+A, Ctrl/Shift+click, Shift+arrows | Select several items |
| Ctrl+Shift+C | Copy path |
| Alt+Enter | Properties |
| Ctrl+T / Ctrl+W | New tab / close tab |
| Ctrl+Tab / Ctrl+Shift+Tab, Ctrl+1…9 | Switch tabs |
| Ctrl+N | New window |
| Ctrl+Enter | Open the selected folder in a new tab |
| Alt+Left / Alt+Right / Backspace, mouse back/forward | Back / forward |
| Alt+Up | Parent folder |
| Ctrl+L / Alt+D / F4 | Edit the address (Tab accepts a suggestion) |
| F5 | Refresh |
| (sidebar) Recycle Bin, or type "Recycle Bin" in the address bar | Browse, restore or permanently delete deleted items |
| Ctrl+H | Show hidden items |
| Arrows, PageUp/PageDown, Home/End, typing | Select; Enter opens |

## Repository layout

| Crate | Purpose |
|-------|---------|
| `crates/lumo-core`     | Platform-agnostic filesystem logic (streaming scanner) |
| `crates/lumo-platform` | OS integration: drives, known folders, opening files |
| `crates/lumo-ui`       | The egui window; builds the `lumo` binary |
| `crates/lumo-cli`      | The `lumo-cli` command-line tool |

## License

[MIT](LICENSE)

Bundled fonts: [IBM Plex Sans](https://github.com/IBM/plex) and
[Lilex](https://github.com/mishamyrt/Lilex), both under the SIL Open Font
License 1.1 (see `crates/lumo-ui/assets/fonts/`).
