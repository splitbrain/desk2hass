//! Main loop: reads the sensors, sends them to Home Assistant and updates the tray.

use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_channel::{Receiver, RecvTimeoutError};
use ksni::blocking::Handle;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::hass::{Client, Values, WebhookGone};
use crate::idle::IdleMonitor;
use crate::tray::{DeskTray, State, Status};
use crate::webcam;

/// Time between two sensor reads.
const TICK: Duration = Duration::from_secs(1);
/// Maximum time between two updates. This is also the heartbeat interval.
const SEND_INTERVAL: Duration = Duration::from_secs(30);
/// Idle time in seconds after which a new user input is sent at once.
const WAKE_THRESHOLD: u64 = 60;

/// Request to the controller.
#[derive(Copy, Clone, Debug)]
pub enum Action {
    /// Stop the agent.
    Quit,
}

/// Owns the sensors, the Home Assistant client and the tray handle.
pub struct Controller {
    tray: Option<Handle<DeskTray>>,
    client: Option<Client>,
    sensors_registered: bool,
    idle: Option<IdleMonitor>,
    state: State,
}

impl Controller {
    /// Create the controller. Without a tray handle the agent runs headless.
    pub fn new(tray: Option<Handle<DeskTray>>) -> Self {
        let idle = match IdleMonitor::connect() {
            Ok(m) => Some(m),
            Err(e) => {
                warn!("idle time not available: {e:#}");
                None
            }
        };
        Self {
            tray,
            client: None,
            sensors_registered: false,
            idle,
            state: State::default(),
        }
    }

    /// Run until a `Quit` action arrives or all senders are gone.
    pub fn run(mut self, actions: Receiver<Action>) {
        let mut last_attempt: Option<Instant> = None;
        let mut sent = Values::default();
        let mut prev_idle: Option<u64> = None;
        loop {
            match actions.recv_timeout(TICK) {
                Ok(Action::Quit) | Err(RecvTimeoutError::Disconnected) => break,
                Err(RecvTimeoutError::Timeout) => {}
            }

            let values = Values {
                idle_secs: self.idle_secs(),
                webcam: webcam::in_use(),
            };
            let woke = matches!(
                (prev_idle, values.idle_secs),
                (Some(before), Some(now)) if before >= WAKE_THRESHOLD && now < WAKE_THRESHOLD
            );
            prev_idle = values.idle_secs;
            let due = last_attempt.map_or(true, |t| t.elapsed() >= SEND_INTERVAL);
            if due || woke || values.webcam != sent.webcam {
                last_attempt = Some(Instant::now());
                self.send(&values);
                sent = values;
            }
        }
        info!("controller stopped");
    }

    /// Read the idle time. Connects to the X server again after a failure.
    fn idle_secs(&mut self) -> Option<u64> {
        if self.idle.is_none() {
            match IdleMonitor::connect() {
                Ok(m) => {
                    info!("connected to X server");
                    self.idle = Some(m);
                }
                Err(e) => debug!("idle time not available: {e:#}"),
            }
        }
        match self.idle.as_ref()?.idle_secs() {
            Ok(secs) => Some(secs),
            Err(e) => {
                warn!("idle time query failed: {e:#}");
                self.idle = None;
                None
            }
        }
    }

    /// Send the values and show the result in the tray.
    fn send(&mut self, values: &Values) {
        if self.client.is_none() {
            match Config::load() {
                Ok(Some(config)) => {
                    self.client = Some(Client::new(&config));
                    self.sensors_registered = false;
                }
                Ok(None) => {
                    self.set_status(
                        Status::NotRegistered,
                        "Not registered, run desk2hass --login".into(),
                    );
                    return;
                }
                Err(e) => {
                    warn!("{e:#}");
                    self.set_status(Status::Error, format!("Configuration error: {e}"));
                    return;
                }
            }
        }

        match self.push(values) {
            Ok(()) => {
                debug!(?values, "update sent");
                self.state.last_update = Some(chrono::Local::now().format("%H:%M:%S").to_string());
                self.state.idle_secs = values.idle_secs;
                self.state.webcam = values.webcam;
                self.set_status(Status::Connected, "Connected".into());
            }
            Err(e) if e.is::<WebhookGone>() => {
                warn!("{e}");
                self.client = None;
                self.set_status(
                    Status::NotRegistered,
                    "Device deleted in Home Assistant, run desk2hass --login".into(),
                );
            }
            Err(e) => {
                warn!("update failed: {e:#}");
                self.set_status(Status::Error, format!("Error: {e}"));
            }
        }
    }

    /// Register the sensors when necessary, else update them.
    fn push(&mut self, values: &Values) -> Result<()> {
        let Some(client) = self.client.as_ref() else {
            return Ok(());
        };
        if self.sensors_registered && client.update(values)? {
            return Ok(());
        }
        client.register_sensors(values)?;
        info!("sensors registered");
        self.sensors_registered = true;
        Ok(())
    }

    /// Set the status and give the new state to the tray.
    fn set_status(&mut self, status: Status, message: String) {
        self.state.status = status;
        self.state.message = message;
        if let Some(tray) = &self.tray {
            let state = self.state.clone();
            tray.update(move |t| t.state = state);
        }
    }
}
