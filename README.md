# Lumo File Explorer

A fast, private, developer-friendly file manager written in Rust, targeting
Windows 10/11 first and Linux next.

> **Status:** early development (Phase 1 done). There is a streaming directory
> scanner and the `lumo ls` CLI; the windowed UI is next.
> See [ROADMAP.md](ROADMAP.md) for the plan and [GOALS.md](GOALS.md) for targets.

## Building

Requirements: [rustup](https://rustup.rs). The toolchain version is pinned in
`rust-toolchain.toml` and installs automatically on first build.

On Windows you also need the MSVC build tools ("Desktop development with C++"
workload from Visual Studio Build Tools).

```sh
cargo build --release          # binary at target/release/lumo(.exe)
cargo test --workspace --all-features    # run all tests
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo bench -p lumo-core --bench folder_load   # scan benchmarks
```

## Usage

```sh
lumo --version
lumo ls [PATH]                 # list a directory (defaults to the current one)
```

## Repository layout

| Crate | Purpose |
|-------|---------|
| `crates/lumo-core` | Platform-agnostic filesystem logic |
| `crates/lumo-cli`  | The `lumo` binary |

Further crates (`lumo-platform`, `lumo-ui`) are added as their roadmap phases begin.

## License

[MIT](LICENSE)
