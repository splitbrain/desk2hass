//! StatusNotifierItem tray icon (via `ksni`).
//!
//! The controller calls `handle.update(|tray| tray.state = …)` after each
//! change. Menu callbacks run on the ksni thread and only send an `Action`
//! through the channel.

use std::sync::Arc;

use crossbeam_channel::Sender;
use ksni::menu::StandardItem;
use ksni::{Icon, MenuItem, OfflineReason, ToolTip, Tray};
use tracing::{debug, warn};

use crate::controller::Action;
use crate::icons::Icons;

/// Connection status to Home Assistant.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Status {
    /// No usable registration. The user must run `--login`.
    #[default]
    NotRegistered,
    /// The last update was accepted.
    Connected,
    /// The last update failed.
    Error,
}

/// Everything that the tray shows.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Connection status.
    pub status: Status,
    /// Text that explains the status.
    pub message: String,
    /// Local time of the last accepted update.
    pub last_update: Option<String>,
    /// Idle time that was sent last.
    pub idle_secs: Option<u64>,
    /// Webcam state that was sent last.
    pub webcam: bool,
}

/// The tray item.
pub struct DeskTray {
    /// Data that the tray shows.
    pub state: State,
    /// Channel to the controller.
    pub actions: Sender<Action>,
    /// Prerendered icons.
    pub icons: Arc<Icons>,
}

impl Tray for DeskTray {
    /// Left click opens the menu, because the menu shows all data.
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "desk2hass".into()
    }

    fn title(&self) -> String {
        "desk2hass".into()
    }

    /// Keep the tray service running while no tray host is present, so that
    /// the icon comes back after a panel restart. A real error ends the
    /// service and the agent continues without an icon.
    fn watcher_offline(&self, reason: OfflineReason) -> bool {
        match reason {
            OfflineReason::Error(e) => {
                warn!("tray service error: {e:?}, no tray icon from now on");
                false
            }
            _ => {
                debug!("no tray host present, waiting for one to register");
                true
            }
        }
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.icons.get(self.state.status)
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            title: "desk2hass".into(),
            description: self.state.message.clone(),
            icon_name: String::new(),
            icon_pixmap: self.icons.get(self.state.status),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let s = &self.state;
        let actions = self.actions.clone();
        vec![
            disabled(s.message.clone()),
            disabled(format!(
                "Last update: {}",
                s.last_update.as_deref().unwrap_or("never")
            )),
            disabled(format!(
                "Idle time: {}",
                s.idle_secs.map_or("unknown".into(), |i| format!("{i} s"))
            )),
            disabled(format!(
                "Webcam: {}",
                if s.webcam { "in use" } else { "not in use" }
            )),
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                activate: Box::new(move |_| {
                    let _ = actions.try_send(Action::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// A menu line that only shows text.
fn disabled<T>(label: String) -> MenuItem<T> {
    StandardItem {
        label,
        enabled: false,
        ..Default::default()
    }
    .into()
}
