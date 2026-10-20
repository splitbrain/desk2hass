//! User idle time from the X11 MIT-SCREEN-SAVER extension.

use anyhow::{Context, Result};
use x11rb::connection::Connection;
use x11rb::protocol::screensaver::ConnectionExt;
use x11rb::rust_connection::RustConnection;

/// Open connection to the X server.
pub struct IdleMonitor {
    conn: RustConnection,
    root: u32,
}

impl IdleMonitor {
    /// Connect to the X server in `$DISPLAY`.
    pub fn connect() -> Result<Self> {
        let (conn, screen) = x11rb::connect(None).context("connect to X server")?;
        let root = conn.setup().roots[screen].root;
        Ok(Self { conn, root })
    }

    /// Seconds since the last keyboard or mouse event.
    pub fn idle_secs(&self) -> Result<u64> {
        let info = self
            .conn
            .screensaver_query_info(self.root)?
            .reply()
            .context("query screen saver info")?;
        Ok(u64::from(info.ms_since_user_input) / 1000)
    }
}
