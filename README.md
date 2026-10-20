# desk2hass

A small Linux agent that sends data from an X11 desktop to Home Assistant.
It uses the Home Assistant mobile app integration. The computer shows as a
device in Home Assistant with these sensors:

| Sensor | Type | Value |
|---|---|---|
| Idle time | sensor | Seconds since the last keyboard or mouse event |
| Last seen | sensor (timestamp) | Time of the last update. Use it to find out if the computer is on |
| Webcam | binary sensor | On when a process has a `/dev/video*` device open |

- One Rust binary with no links to X11, GTK or D-Bus libraries.
- The idle time comes from the X11 MIT-SCREEN-SAVER extension.
- Tray icon through [`ksni`](https://crates.io/crates/ksni) (StatusNotifierItem).

## Build

```
cargo build --release
```

## Install

```
install -Dm755 target/release/desk2hass ~/.local/bin/desk2hass
install -Dm644 packaging/autostart/desk2hass.desktop ~/.config/autostart/
```

## Login

Make a long-lived access token in Home Assistant (Profile, Security). Then
register the computer:

```
desk2hass --login https://homeassistant.local:8123 <TOKEN>
```

The registration data goes into `~/.config/desk2hass/config.json`. The token
is only necessary for the registration and is not kept.

If Home Assistant uses a self-signed certificate, you have two options:

- Add the certificate to the certificate store of the system.
- Add `--insecure` to the login command. The agent then accepts all
  certificates.

If you delete the device in Home Assistant, do the login again.

## Run

```
desk2hass [OPTIONS]

  --login <URL> <TOKEN>   Register in Home Assistant and exit
  --insecure              With --login: accept all TLS certificates
  --log-level <LEVEL>     trace, debug, info, warn or error (default: info)
  --no-tray               Run without a tray icon
```

`RUST_LOG` (for example `RUST_LOG=desk2hass=debug`) overrides `--log-level`.

## Updates

The agent reads the sensors each second. It sends an update:

- every 30 seconds,
- at once when the webcam state changes,
- at once when new input comes after an idle time of 60 seconds or more.

## Tray

The icon color shows the status:

- blue: connected
- red: the last update failed
- grey: not registered

A left click opens the registered Home Assistant in the default browser
(with `xdg-open`). A right click opens the menu. The menu shows the status,
the time of the last update and the values that were sent last. It also has
an item that opens Home Assistant.

XFCE: The icon needs the "Status Tray" plugin (XFCE 4.16 and later) or the
"StatusNotifier" plugin in the panel. The "Notification Area" plugin does not
show it.

## Limits

- The webcam sensor only sees processes that the current user can inspect.
- Only X11 is supported. Wayland has no general API for the idle time.

## License

MIT.
