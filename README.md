# Fastop

A *fast, lightweight* alternative to [btop](https://github.com/aristocratos/btop), built in Rust with [Ratatui](https://ratatui.rs)!

## Features

- CPU panel with per-core bars, a package temperature gauge and a history graph
- Memory and swap meters
- GPU usage plus a temperature gauge and history graph (sysfs, with an `nvidia-smi` fallback)
- Disk usage per physical device
- Network throughput per interface (download/upload rates)
- Battery charge, state and time remaining (shown when a battery is present)
- Process table with selection and sortable columns (`s` cycles CPU → MEM → PID → NAME)
- Responsive layout: full 2×2 grid on large terminals, a single compact row on smaller ones

## Usage

```sh
fastop [OPTIONS]
```

| Option | Description | Default |
| --- | --- | --- |
| `-t, --tick <MS>` | Refresh interval in milliseconds (50–60000) | config file, then `500` |
| `-l, --layout <LAYOUT>` | Panel layout: `auto`, `compact`, or `grid` | config file, then `auto` |
| `-c, --config <PATH>` | Use an alternative configuration file | platform config path |
| `--write-config` | Write the effective configuration to the config file and exit | |
| `-h, --help` | Print help | |
| `-V, --version` | Print version | |

## Configuration

Fastop reads `config.toml` from the platform-specific configuration directory,
keeping values out of the command line. Command-line options take precedence
over the file, which takes precedence over the built-in defaults.

| Platform | Path |
| --- | --- |
| Linux | `~/.config/fastop/config.toml` |
| macOS | `~/Library/Application Support/fastop/config.toml` |
| Windows | `%APPDATA%\fastop\config.toml` |

The directory is created on demand and the file is optional; when it is missing
Fastop runs with sensible defaults. The format is a small TOML document where
every key is optional:

```toml
# Refresh interval in milliseconds (50–60000)
tick = 500

# Panel layout: "auto", "compact", or "grid"
layout = "auto"
```

Invalid TOML is reported on stderr and Fastop falls back to defaults. Run
`fastop --write-config` to create or update the file; existing keys that Fastop
does not manage are preserved. Use `-c, --config <PATH>` to point at a
different file.

### Key bindings

| Key | Action |
| --- | --- |
| `↑` / `k` | Select previous process |
| `↓` / `j` | Select next process |
| `s` | Cycle the process sort column |
| `q` / `Esc` | Quit |

## Development

```sh
cargo run            # run the TUI
cargo test           # run the test suite
cargo clippy --all-targets -- -D warnings
cargo fmt
```

### Project layout

| Module | Responsibility |
| --- | --- |
| `main.rs` | Entry point: parse CLI options, load config and start the app |
| `cli.rs` | `clap` argument definitions |
| `config.rs` | Cross-platform configuration file loading and saving |
| `app.rs` | Application state, event loop and top-level render
| `panels.rs` | Rendering for each panel (header, footer, CPU, memory, …)
| `layout.rs` | Panel selection and responsive layout decisions |
| `theme.rs` | Color constants and threshold-based coloring |
| `widgets.rs` | Reusable UI primitives (panels, meters, bars) |
| `format.rs` | Byte/rate/percentage formatting helpers |
| `processes.rs` | Process rows, columns and sorting |
| `disks.rs` | Disk enumeration and filtering |
| `network.rs` | Network interface filtering and rows |
| `gpu.rs` | GPU detection and sampling |
| `battery.rs` | Battery detection and snapshots |
| `temperature.rs` | Rolling temperature history for the graphs |
| `sensors.rs` | CPU temperature selection |
