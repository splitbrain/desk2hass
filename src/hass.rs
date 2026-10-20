//! Home Assistant mobile app integration: registration and sensor webhook.

use std::fmt;
use std::fs;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use ureq::tls::{RootCerts, TlsConfig};
use ureq::Agent;

use crate::config::Config;

/// Unique ID of the idle time sensor.
pub const IDLE_TIME: &str = "idle_time";
/// Unique ID of the heartbeat sensor.
pub const LAST_SEEN: &str = "last_seen";
/// Unique ID of the webcam sensor.
pub const WEBCAM: &str = "webcam";

/// Home Assistant deleted the device of this webhook.
#[derive(Debug)]
pub struct WebhookGone;

impl fmt::Display for WebhookGone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("device was deleted in Home Assistant")
    }
}

impl std::error::Error for WebhookGone {}

/// Reply of the registration endpoint.
#[derive(Deserialize)]
struct Registration {
    webhook_id: String,
}

/// Register this computer as a mobile app device and return its configuration.
pub fn register(url: &str, token: &str, insecure: bool) -> Result<Config> {
    let url = url.trim_end_matches('/').to_string();
    let body = json!({
        "device_id": device_id()?,
        "app_id": "desk2hass",
        "app_name": "desk2hass",
        "app_version": env!("CARGO_PKG_VERSION"),
        "device_name": read_trimmed("/proc/sys/kernel/hostname").unwrap_or_else(|| "Linux".into()),
        "manufacturer": read_trimmed("/sys/class/dmi/id/sys_vendor").unwrap_or_else(|| "Unknown".into()),
        "model": read_trimmed("/sys/class/dmi/id/product_name").unwrap_or_else(|| "Unknown".into()),
        "os_name": "Linux",
        "os_version": read_trimmed("/proc/sys/kernel/osrelease").unwrap_or_default(),
        "supports_encryption": false,
        "app_data": {},
    });
    let mut resp = agent(insecure)
        .post(format!("{url}/api/mobile_app/registrations"))
        .header("Authorization", format!("Bearer {token}"))
        .send_json(&body)
        .context("send registration")?;
    let status = resp.status().as_u16();
    if status != 201 && status != 200 {
        let text = resp.body_mut().read_to_string().unwrap_or_default();
        bail!("registration failed with HTTP {status}: {text}");
    }
    let reg: Registration = resp
        .body_mut()
        .read_json()
        .context("parse registration reply")?;
    Ok(Config {
        url,
        webhook_id: reg.webhook_id,
        insecure,
    })
}

/// Current sensor values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Values {
    /// Seconds since the last user input, `None` when unknown.
    pub idle_secs: Option<u64>,
    /// True when a video device is open.
    pub webcam: bool,
}

/// Webhook client for a registered device.
pub struct Client {
    agent: Agent,
    webhook_url: String,
}

impl Client {
    /// Create a client for the registration in `config`.
    pub fn new(config: &Config) -> Self {
        Self {
            agent: agent(config.insecure),
            webhook_url: format!("{}/api/webhook/{}", config.url, config.webhook_id),
        }
    }

    /// Register all sensors. Sensors that exist already get updated.
    pub fn register_sensors(&self, values: &Values) -> Result<()> {
        let sensors = [
            json!({
                "type": "sensor",
                "unique_id": IDLE_TIME,
                "name": "Idle time",
                "state": values.idle_secs,
                "device_class": "duration",
                "unit_of_measurement": "s",
                "state_class": "measurement",
                "icon": "mdi:timer-sand",
            }),
            json!({
                "type": "sensor",
                "unique_id": LAST_SEEN,
                "name": "Last seen",
                "state": now_rfc3339(),
                "device_class": "timestamp",
                "entity_category": "diagnostic",
                "icon": "mdi:heart-pulse",
            }),
            json!({
                "type": "binary_sensor",
                "unique_id": WEBCAM,
                "name": "Webcam",
                "state": values.webcam,
                "icon": webcam_icon(values.webcam),
            }),
        ];
        for sensor in sensors {
            self.call("register_sensor", sensor)?;
        }
        Ok(())
    }

    /// Send all sensor values. Returns false when Home Assistant does not
    /// know one of the sensors, so that the caller registers them again.
    pub fn update(&self, values: &Values) -> Result<bool> {
        let data = json!([
            { "type": "sensor", "unique_id": IDLE_TIME, "state": values.idle_secs },
            { "type": "sensor", "unique_id": LAST_SEEN, "state": now_rfc3339() },
            {
                "type": "binary_sensor",
                "unique_id": WEBCAM,
                "state": values.webcam,
                "icon": webcam_icon(values.webcam),
            },
        ]);
        let reply = self.call("update_sensor_states", data)?;
        let registered = reply
            .as_object()
            .map(|sensors| {
                sensors.values().all(|s| {
                    s.pointer("/error/code").and_then(Value::as_str) != Some("not_registered")
                })
            })
            .unwrap_or(true);
        Ok(registered)
    }

    /// Send one webhook request and return the JSON reply.
    fn call(&self, kind: &str, data: Value) -> Result<Value> {
        let mut resp = self
            .agent
            .post(&self.webhook_url)
            .send_json(json!({ "type": kind, "data": data }))
            .with_context(|| format!("send {kind}"))?;
        match resp.status().as_u16() {
            200..=299 => {}
            410 => return Err(WebhookGone.into()),
            status => bail!("{kind} failed with HTTP {status}"),
        }
        let text = resp.body_mut().read_to_string()?;
        if text.trim().is_empty() {
            return Ok(Value::Null);
        }
        serde_json::from_str(&text).with_context(|| format!("parse {kind} reply"))
    }
}

/// HTTP agent that returns error status codes as normal responses.
fn agent(insecure: bool) -> Agent {
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::PlatformVerifier)
        .disable_verification(insecure)
        .build();
    Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(10)))
        .http_status_as_error(false)
        .build()
        .into()
}

/// Stable device ID, derived from the systemd machine ID.
fn device_id() -> Result<String> {
    let id = read_trimmed("/etc/machine-id").context("read /etc/machine-id")?;
    Ok(format!("desk2hass-{id}"))
}

/// Content of a small system file without surrounding white space.
fn read_trimmed(path: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// Current time in the format that Home Assistant expects for timestamps.
fn now_rfc3339() -> String {
    chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, false)
}

/// Icon of the webcam sensor for the given state.
fn webcam_icon(in_use: bool) -> &'static str {
    if in_use {
        "mdi:webcam"
    } else {
        "mdi:webcam-off"
    }
}
