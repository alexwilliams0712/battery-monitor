# battery-monitor

Tray icon showing battery levels of wireless peripherals,
such as mice, keyboards and headsets.
A Rust port of the idea behind
[Wireless-Device-Battery-Monitor](https://github.com/krisgfx9/Wireless-Device-Battery-Monitor)
for Ubuntu and macOS.

## Ubuntu

The kernel `hid-logitech-hidpp` driver already reads HID++ battery data and
UPower republishes it, so this app only reads UPower over D-Bus.
Any other peripheral UPower knows about, such as a Bluetooth headset,
shows up too.
No udev rules or root access are needed.

- The top bar shows each device and its level, e.g. `G502 12%  G915 TKL 37%`.
  With more than four devices it shows only the percentages.
- The icon shows the lowest battery.
- The menu lists every device with its charging state.
- Levels refresh every 30 seconds.

Install the `.deb` from the
[latest release](https://github.com/alexwilliams0712/battery-monitor/releases/latest):

```sh
sudo apt install ./battery-monitor_*_amd64.deb
battery-monitor &
```

It starts automatically at every login after that.

To build from source instead:

```sh
cargo install --git https://github.com/alexwilliams0712/battery-monitor
sed "s|^Exec=.*|Exec=$HOME/.cargo/bin/battery-monitor|" battery-monitor.desktop \
  > ~/.config/autostart/battery-monitor.desktop
```

Check what UPower sees with `upower -d`.
`battery-monitor --hidpp` prints what the macOS HID++ backend reads over
hidraw, for comparison.
Levels from battery voltage can differ by a point, because the kernel and
Solaar use different discharge curves.

## macOS

macOS has no kernel HID++ support, so the app talks HID++ 2.0 to Logitech
Unifying, Lightspeed and Bolt receivers itself (battery features 0x1004,
0x1000 and 0x1001, as Solaar does).
The menu bar shows the same text as on Ubuntu, and its menu lists every
device and Quit.
Devices paired directly over Bluetooth are not read yet.

```sh
cargo install --path .
battery-monitor --hidpp   # read once and print, to check access
battery-monitor &
```

If no devices appear, allow the binary (or the terminal that starts it) in
System Settings > Privacy & Security > Input Monitoring, then restart it.

To start it at login, add the binary in System Settings > General >
Login Items, or load a LaunchAgent:

```sh
cat > ~/Library/LaunchAgents/battery-monitor.plist <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>battery-monitor</string>
  <key>ProgramArguments</key><array><string>$HOME/.cargo/bin/battery-monitor</string></array>
  <key>RunAtLoad</key><true/>
</dict>
</plist>
EOF
launchctl load ~/Library/LaunchAgents/battery-monitor.plist
```

Next steps: a signed and notarised app bundle, so the Input Monitoring grant
survives rebuilds, and a Homebrew cask.

## Releasing

Bump `version` in `Cargo.toml`, then push a matching `v*` tag.
The release workflow builds the `.deb` and attaches it to a GitHub release.

## Licence

[PolyForm Strict 1.0.0](LICENSE.md).
You may view the source and use the software for any noncommercial purpose.
You may not modify it, build new works from it, or redistribute it.
