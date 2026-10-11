mod app;
mod battery;
mod cli;
mod collect;
mod config;
mod disks;
mod format;
mod gpu;
mod layout;
mod network;
mod processes;
mod sensors;
mod temperature;
mod theme;
mod ui;
mod widgets;

use std::path::Path;

use clap::Parser;

use crate::app::App;
use crate::cli::Cli;
use crate::collect::CollectOptions;
use crate::config::{Config, Overrides};

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    if cli.version {
        println!("fastop {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if cli.config_path {
        return print_config_path(cli.config.as_deref());
    }

    let overrides = Overrides {
        tick: cli.tick,
        layout: cli.layout,
        sort: cli.sort,
        gpu: cli.no_gpu.then_some(false),
        battery: cli.no_battery.then_some(false),
    };

    let config = match Config::load(cli.config.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("fastop: {error}; using defaults");
            Config::default()
        }
    };
    let config = config.with_overrides(overrides);

    if cli.write_config {
        match config.save(cli.config.as_deref()) {
            Ok(path) => println!("Wrote configuration to {}", path.display()),
            Err(error) => {
                eprintln!("fastop: {error}");
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    let options = CollectOptions {
        gpu: config.gpu,
        battery: config.battery,
    };

    ratatui::run(|terminal| {
        App::new(config.tick_rate(), config.layout, config.sort, options).run(terminal)
    })
}

/// Prints the configuration file that Fastop would use and returns.
///
/// An explicit `--config` path is echoed verbatim; otherwise the platform
/// specific location is resolved.
fn print_config_path(configured: Option<&Path>) -> std::io::Result<()> {
    let path = match configured {
        Some(path) => path.to_path_buf(),
        None => match Config::path() {
            Ok(path) => path,
            Err(error) => {
                eprintln!("fastop: {error}");
                std::process::exit(1);
            }
        },
    };

    println!("{}", path.display());
    Ok(())
}
