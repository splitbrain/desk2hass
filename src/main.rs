//! desk2hass: sends the idle time and the webcam use of a Linux desktop to Home Assistant.

mod config;
mod controller;
mod hass;
mod icons;
mod idle;
mod tray;
mod webcam;

use std::sync::Arc;
use std::thread;

use anyhow::Result;
use clap::Parser;
use ksni::blocking::TrayMethods;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::controller::{Action, Controller};
use crate::tray::DeskTray;

/// Command line options.
#[derive(Parser, Debug)]
#[command(name = "desk2hass", version)]
struct Cli {
    /// Register this computer in Home Assistant with a long-lived access token and exit.
    #[arg(long, num_args = 2, value_names = ["URL", "TOKEN"])]
    login: Option<Vec<String>>,

    /// With --login: accept any TLS certificate, for example a self-signed one.
    #[arg(long, default_value_t = false, requires = "login")]
    insecure: bool,

    /// Logging filter: trace, debug, info, warn, error.
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Run without a tray icon.
    #[arg(long, default_value_t = false)]
    no_tray: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(format!("desk2hass={}", cli.log_level)));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();

    if let Some(login) = cli.login {
        let config = hass::register(&login[0], &login[1], cli.insecure)?;
        let path = config.save()?;
        println!("Registered. Configuration written to {}", path.display());
        return Ok(());
    }

    info!(version = env!("CARGO_PKG_VERSION"), "desk2hass starting");

    let (tx, rx) = crossbeam_channel::unbounded::<Action>();
    ctrlc::set_handler({
        let tx = tx.clone();
        move || {
            info!("SIGINT/SIGTERM received");
            let _ = tx.try_send(Action::Quit);
        }
    })?;

    let tray_handle = if cli.no_tray {
        None
    } else {
        let tray = DeskTray {
            state: Default::default(),
            actions: tx.clone(),
            icons: Arc::new(icons::Icons::new()),
        };
        match tray.assume_sni_available(true).spawn() {
            Ok(h) => Some(h),
            Err(e) => {
                warn!("tray service failed: {e:?}, running without tray icon");
                None
            }
        }
    };

    let controller = Controller::new(tray_handle);
    let handle = thread::Builder::new()
        .name("controller".into())
        .spawn(move || controller.run(rx))?;
    let _ = handle.join();

    info!("desk2hass exited");
    Ok(())
}
