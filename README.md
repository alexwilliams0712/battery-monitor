# battery-monitor

Shows wireless peripheral battery levels in the Ubuntu top bar or macOS menu bar,
e.g. `G502 12%  G915 TKL 37%`.
With more than four devices it shows only the percentages.
The menu lists each device with its charging state.

## Ubuntu

Shows every peripheral battery UPower reports.

```sh
sudo apt install ./battery-monitor_*_amd64.deb   # from the latest release
battery-monitor &
```

It starts at every login after that.

## macOS

Reads Logitech Unifying, Lightspeed and Bolt receivers over HID++.

```sh
cargo install --git https://github.com/alexwilliams0712/battery-monitor
battery-monitor --hidpp   # read once and print
battery-monitor &
```

If no devices appear, allow it in Privacy & Security > Input Monitoring.
Add it to Login Items to start it at login.

## Releasing

Bump `version` in `Cargo.toml`, then push a matching `v*` tag.

## Licence

[PolyForm Strict 1.0.0](LICENSE.md).
