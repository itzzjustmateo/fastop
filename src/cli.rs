use std::path::PathBuf;

use clap::{ArgAction, Parser, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::processes::SortBy;

/// Default refresh interval in milliseconds.
pub(crate) const DEFAULT_TICK_MS: u64 = 500;

/// Smallest accepted refresh interval in milliseconds.
pub(crate) const MIN_TICK_MS: u64 = 50;

/// Largest accepted refresh interval in milliseconds.
pub(crate) const MAX_TICK_MS: u64 = 60_000;

const LONG_ABOUT: &str = "\
A fast, lightweight terminal system monitor.

Live CPU, memory, GPU, disk, network, battery and process information is
rendered with Ratatui. Settings can be supplied on the command line, in the
configuration file, or neither, in which case the built-in defaults apply.
Command-line options take precedence over the configuration file.";

const EXAMPLES: &str = "\
Examples:
  fastop                        Start with the defaults
  fastop --tick 250ms           Refresh every 250 milliseconds
  fastop -l grid -s memory      Force the grid layout, sort processes by memory
  fastop --no-gpu               Skip GPU detection
  fastop --write-config         Save the effective configuration";

/// A fast, lightweight alternative to btop.
#[derive(Debug, Parser)]
#[command(
    name = "fastop",
    version,
    about = "A fast, lightweight terminal system monitor",
    long_about = LONG_ABOUT,
    after_help = EXAMPLES,
    disable_version_flag = true
)]
pub(crate) struct Cli {
    /// Print version information and exit.
    #[arg(
        short = 'v',
        visible_short_alias = 'V',
        long = "version",
        action = ArgAction::SetTrue
    )]
    pub(crate) version: bool,

    /// Print the resolved configuration file path and exit.
    #[arg(long, action = ArgAction::SetTrue)]
    pub(crate) config_path: bool,

    /// Refresh interval: milliseconds, or a duration like `500ms`, `2s`, `1m`.
    ///
    /// Overrides the value from the configuration file. [default: 500]
    #[arg(short, long, value_name = "DURATION", value_parser = parse_tick_ms)]
    pub(crate) tick: Option<u64>,

    /// Panel layout to use.
    ///
    /// Overrides the value from the configuration file. [default: auto]
    #[arg(short, long, value_enum)]
    pub(crate) layout: Option<LayoutMode>,

    /// Initial process-table sort column.
    ///
    /// Overrides the value from the configuration file. [default: cpu]
    #[arg(short, long, value_enum, value_name = "COLUMN")]
    pub(crate) sort: Option<SortBy>,

    /// Do not detect or sample a GPU.
    #[arg(long)]
    pub(crate) no_gpu: bool,

    /// Do not detect or sample the battery.
    #[arg(long)]
    pub(crate) no_battery: bool,

    /// Path to an alternative configuration file.
    #[arg(short, long, value_name = "PATH")]
    pub(crate) config: Option<PathBuf>,

    /// Write the effective configuration to the config file and exit.
    #[arg(long)]
    pub(crate) write_config: bool,
}

/// Parses a refresh interval into whole milliseconds.
///
/// A bare number is interpreted as milliseconds for backwards compatibility;
/// the `ms`, `s` and `m` suffixes are also accepted. The result is validated
/// against [`MIN_TICK_MS`] and [`MAX_TICK_MS`].
fn parse_tick_ms(value: &str) -> Result<u64, String> {
    let value = value.trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number, 1)
    } else if let Some(number) = value.strip_suffix('s') {
        (number, 1_000)
    } else if let Some(number) = value.strip_suffix('m') {
        (number, 60_000)
    } else {
        (value, 1)
    };

    let number: u64 = number.trim().parse().map_err(|_| {
        format!("`{value}` is not a valid interval (try `500`, `500ms`, `2s` or `1m`)")
    })?;
    let milliseconds = number
        .checked_mul(multiplier)
        .ok_or_else(|| format!("`{value}` is too large (maximum is {MAX_TICK_MS} ms)"))?;

    if !(MIN_TICK_MS..=MAX_TICK_MS).contains(&milliseconds) {
        return Err(format!(
            "`{value}` is out of range ({MIN_TICK_MS} ms to {MAX_TICK_MS} ms)"
        ));
    }

    Ok(milliseconds)
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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn parses_plain_milliseconds() {
        assert_eq!(parse_tick_ms("500"), Ok(500));
        assert_eq!(parse_tick_ms(" 250 "), Ok(250));
    }

    #[test]
    fn parses_suffixed_durations() {
        assert_eq!(parse_tick_ms("500ms"), Ok(500));
        assert_eq!(parse_tick_ms("2s"), Ok(2_000));
        assert_eq!(parse_tick_ms("1m"), Ok(60_000));
    }

    #[test]
    fn rejects_bad_or_out_of_range_values() {
        assert!(parse_tick_ms("fast").is_err());
        assert!(parse_tick_ms("10").is_err());
        assert!(parse_tick_ms("2m").is_err());
        assert!(parse_tick_ms("").is_err());
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_tick_and_sort_arguments() {
        let cli = Cli::try_parse_from(["fastop", "--tick", "2s", "--sort", "memory", "--no-gpu"])
            .expect("valid arguments");

        assert_eq!(cli.tick, Some(2_000));
        assert_eq!(cli.sort, Some(SortBy::Memory));
        assert!(cli.no_gpu);
        assert!(!cli.no_battery);
        assert!(!cli.version);
    }
}
