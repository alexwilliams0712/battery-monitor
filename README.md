# logi-battery-tray

Tray icon showing battery levels of wireless peripherals, such as Logitech
Lightspeed and Unifying devices.
A Rust port of the idea behind
[Wireless-Device-Battery-Monitor](https://github.com/krisgfx9/Wireless-Device-Battery-Monitor)
for Ubuntu and macOS.

## Ubuntu

The kernel `hid-logitech-hidpp` driver already reads HID++ battery data and
UPower republishes it, so this app only reads UPower over D-Bus.
No udev rules or root access are needed.

- The top bar shows each device and its level, e.g. `G502 12%  G915 TKL 37%`.
  With more than four devices it shows only the percentages.
- The icon shows the lowest battery.
- The menu lists every device with its charging state.
- Levels refresh every 30 seconds.

Install the `.deb` from the
[latest release](https://github.com/alexwilliams0712/logi-battery-tray/releases/latest):

```sh
sudo apt install ./logi-battery-tray_*_amd64.deb
logi-battery-tray &
```

It starts automatically at every login after that.

To build from source instead:

```sh
cargo install --git https://github.com/alexwilliams0712/logi-battery-tray
sed "s|^Exec=.*|Exec=$HOME/.cargo/bin/logi-battery-tray|" logi-battery-tray.desktop \
  > ~/.config/autostart/logi-battery-tray.desktop
```

Check what UPower sees with `upower -d`.
A device missing there needs a direct HID++ backend over hidraw.

## macOS

Planned.
macOS has no kernel HID++ support, so it needs a direct HID++ backend
(`hidapi`, features 0x1000/0x1001/0x1004) and a signed app bundle for the
Input Monitoring permission.

## Releasing

Bump `version` in `Cargo.toml`, then push a matching `v*` tag.
The release workflow builds the `.deb` and attaches it to a GitHub release.

## Licence

[PolyForm Strict 1.0.0](LICENSE.md).
You may view the source and use the software for any noncommercial purpose.
You may not modify it, build new works from it, or redistribute it.
