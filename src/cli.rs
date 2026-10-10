use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

/// Default refresh interval in milliseconds.
pub(crate) const DEFAULT_TICK_MS: u64 = 500;

/// Smallest accepted refresh interval in milliseconds.
pub(crate) const MIN_TICK_MS: u64 = 50;

/// Largest accepted refresh interval in milliseconds.
pub(crate) const MAX_TICK_MS: u64 = 60_000;

/// A fast, lightweight alternative to btop.
#[derive(Debug, Parser)]
#[command(name = "fastop", version, about, long_about = None)]
pub(crate) struct Cli {
    /// Refresh interval in milliseconds.
    ///
    /// Overrides the value from the configuration file. [default: 500]
    #[arg(
        short,
        long,
        value_name = "MS",
        value_parser = clap::value_parser!(u64).range(MIN_TICK_MS..=MAX_TICK_MS),
    )]
    pub(crate) tick: Option<u64>,

    /// Panel layout to use.
    ///
    /// Overrides the value from the configuration file. [default: auto]
    #[arg(short, long, value_enum)]
    pub(crate) layout: Option<LayoutMode>,

    /// Path to an alternative configuration file.
    #[arg(short, long, value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,

    /// Write the effective configuration to the config file and exit.
    #[arg(long)]
    pub(crate) write_config: bool,
}

/// Preferred arrangement of the summary panels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "lowercase")]
pub(crate) enum LayoutMode {
    /// Pick the best layout for the current terminal size.
    Auto,
    /// Always use the single compact row of panels.
    Compact,
    /// Prefer the full panel grid, tightened to fit smaller terminals.
    Grid,
}
