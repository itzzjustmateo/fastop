mod app;
mod cli;
mod config;
mod disks;
mod format;
mod gpu;
mod layout;
mod network;
mod panels;
mod processes;
mod sensors;
mod theme;
mod widgets;

use clap::Parser;

use crate::app::App;
use crate::cli::Cli;
use crate::config::Config;

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    let config = match Config::load(cli.config.as_deref()) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("fastop: {error}; using defaults");
            Config::default()
        }
    };
    let config = config.with_overrides(cli.tick, cli.layout);

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

    ratatui::run(|terminal| App::new(config.tick_rate(), config.layout).run(terminal))
}
