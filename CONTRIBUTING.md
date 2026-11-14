# Contributing to WebCa

Thank you for helping. This page covers the layout of the code, the checks every change must pass
and how releases are made.

## Layout

```
crates/
  webca-core      platform-independent model: controls, ranges, profiles, the settings editor,
                  the agent's decision logic, CLI parsing, the MCP server, translations, updates
  webca-windows   Windows backend: Media Foundation, DirectShow control interfaces, camera
                  defaults store, activity monitors, preview capture, registry and packaging helpers
  webca           the application: GPUI window, tray, background agent, command line
packaging/        MSIX manifest template
scripts/          packaging script
design/           interface references (open with FigCraft)
docs/             notes such as Windows 10 compatibility gaps
```

Logic lives in `webca-core` whenever it does not need Windows. Code in `webca-windows` that calls
into the operating system stays thin; anything that can be computed without a system call goes into
its pure modules (`mapping.rs`, `frame.rs`).

## Requirements

- Windows 11 and a stable Rust toolchain (see `rust-version` in `Cargo.toml`)
- `rustup component add clippy rustfmt llvm-tools-preview`
- `cargo install cargo-llvm-cov`
- A webcam for the hardware tests

## Checks

Every commit must pass, in this order:

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
cargo cov
```

`cargo cov` is an alias defined in `.cargo/config.toml`. It measures `webca-core` and the pure
modules of `webca-windows` and fails below **100 % line and region coverage**. Files that only wrap
operating-system calls are excluded from the gate and covered by hardware tests instead.

### Hardware tests

Tests that need a real camera are marked `#[ignore]` so that CI can run without one:

```
cargo test -p webca-windows -- --ignored --test-threads=1
```

Each test that writes to the camera restores the original value before it finishes.

## Commit messages

```
feat: WC-42 Add exposure presets
eng: WC-43 Speed up preview conversion
```

- `feat` for user-visible changes, `eng` for engineering work (tooling, refactoring, CI)
- `WC-N` is the task number
- a short imperative description, about five words

## Packaging

```
./scripts/package.ps1 -Mode Register
```

builds a release, lays out an MSIX package in `target/package/layout` and registers it for the
current user. This needs **Developer Mode** (Settings › System › For developers). Start the
registered app from the Start menu to test system-wide mode, which only works with package identity.
Remove it with `Get-AppxPackage WebCa | Remove-AppxPackage`.

```
./scripts/package.ps1 -Mode Pack -Publisher "CN=..."
```

produces an unsigned `.msix` (requires the Windows SDK). The publisher must match the subject of the
certificate that will sign it.

## Releases

1. Update the version in `Cargo.toml` and the changelog.
2. Tag the commit: `git tag v0.2.0` and push the tag.
3. The **Release** workflow builds the MSIX package and the portable archive and opens a draft
   release with both.
4. Sign the `.msix` with the project's code-signing certificate, replace the asset in the draft and
   publish it. Unsigned MSIX packages cannot be installed outside Developer Mode.

The update check in the app reads the latest published release of the repository named in the
`repository` field of the workspace `Cargo.toml`; without that field the check is disabled.
