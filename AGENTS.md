# AGENTS.md

Guidance for AI coding agents (and humans) working in this repository.

## Project

**fastop** is a fast, lightweight terminal system monitor built in Rust with
[ratatui](https://ratatui.rs) — an alternative to
[btop](https://github.com/aristocratos/btop). It renders live CPU, memory, GPU,
disk, network, battery and process information as a TUI.

- Binary crate (no library target), Rust **edition 2024**, single entry point
  `src/main.rs`.
- Cross-platform: Linux, macOS and Windows are all supported and exercised in CI.

## Toolchain

- Rust stable. CI uses `dtolnay/rust-toolchain@stable`.
- **Keep `Cargo.lock` committed** — this is a binary, not a library.

## Commands

```sh
cargo run                                                          # run the TUI
cargo fmt --all                                                   # format
cargo build --locked                                              # build like CI
cargo test --locked                                               # run the tests
cargo clippy --all-targets --all-features --locked -- -D warnings # lint (CI gate)
```

Run all of these before pushing. `.github/workflows/rust.yml` enforces:

1. `cargo fmt --all -- --check`
2. `cargo clippy --all-targets --all-features --locked -- -D warnings`
3. `cargo build --locked` and `cargo test --locked` on ubuntu, macOS and Windows.

## Lints

`Cargo.toml` configures:

```toml
[lints.rust]
unsafe_code = "forbid"

[lints.clippy]
dbg_macro = "deny"
todo = "deny"
unimplemented = "deny"
```

Never introduce `unsafe`, `dbg!`, `todo!` or `unimplemented!`. Warnings are
errors in CI, so an unused import or field will fail the build.

## Architecture

| Module | Responsibility |
| --- | --- |
| `main.rs` | Entry point: parse CLI options, load config, start the app |
| `cli.rs` | `clap` definitions (`Cli`, `LayoutMode`, tick bounds) |
| `config.rs` | Cross-platform `config.toml` loading and saving |
| `app.rs` | Application state, event loop, top-level frame layout |
| `panels.rs` | `impl App` renderers for the header, footer and every panel |
| `layout.rs` | Panel selection and responsive layout decisions |
| `theme.rs` | Color constants and threshold-based colors |
| `widgets.rs` | Reusable UI primitives (panels, meters, bars) |
| `format.rs` | Byte / rate / percentage formatting helpers |
| `processes.rs` | Process rows, columns and sorting |
| `disks.rs` | Disk enumeration and filtering |
| `network.rs` | Network interface filtering and rows |
| `gpu.rs` | GPU detection and sampling (sysfs / `nvidia-smi`) |
| `battery.rs` | Battery detection and snapshots |
| `temperature.rs` | Rolling temperature history for the graphs |
| `sensors.rs` | CPU temperature selection |

**Data flow**

1. `main.rs` parses the CLI, loads/merges config, then runs `App`.
2. `App::on_tick` refreshes `sysinfo` handles (system, components, disks,
   networks, users), the GPU backend and the battery snapshot, then rebuilds the
   process and network rows.
3. `App::render` lays out the frame — header, panels band, process table, footer
   — and delegates to the `render_*` methods.

Keep new data collection in its own small module, and new UI in `panels.rs`
(rendering lives there as `impl App` over `pub(crate)` fields). Rendering methods
are pure: they only read state.

## Conventions

- Prefer small, focused modules with `pub(crate)` visibility; avoid `pub`.
- Favour a functional/immutable style; avoid needless `clone`.
- Unit tests live in a `#[cfg(test)] mod tests` at the bottom of each module.
- Add tests for new pure logic (formatting, parsing, layout decisions).
- Document items with `///` doc comments; the crate documents itself this way.
- Never change default runtime behaviour (auto layout, 500 ms tick) without a
  clear reason.

## Layout specifics

- `layout.rs` chooses between the **grid** and the single **compact** row.
- The grid band is 22 rows: `CPU | Memory` (9), `GPU | Disks` (7) and a
  full-width `Network` row (6).
- The battery is a **header indicator**, not a panel.
- Defaults: `tick = 500 ms`, `layout = auto`.

## Platform notes

- **GPU**: read metrics from **sysfs**, falling back to `nvidia-smi`. Do **not**
  use NVML or add a CUDA/NVML dependency.
- **CPU temperature**: pick the most representative sensor by label scoring in
  `sensors.rs`.
- **Battery**: provided by the `starship-battery` crate (pure sysfs / IOKit /
  Windows APIs, no system libraries).
- **Config**: resolve paths with `directories::BaseDirs`; never hardcode home
  directories. Precedence is CLI → `config.toml` → built-in defaults. A missing
  or invalid config must never be fatal — report and fall back to defaults.

## Dependency policy

Keep the dependency tree small. Adding a crate is a design decision: prefer one
well-maintained crate that supports all three CI platforms over hand-rolled
per-OS code, and avoid crates that require system libraries.

## Testing

- Tests must pass on Linux, macOS and Windows and must not assume the host has a
  GPU, a battery, or specific temperature sensors (the CI runners have none).
- Prefer testing pure helpers over I/O and hardware access.

## Git

- Keep commits focused and make sure CI stays green.
- Commit `Cargo.lock` when dependencies change.
- Do not commit local AI-tooling directories (see `.gitignore`).
