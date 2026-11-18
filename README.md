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

- The icon shows the lowest battery.
- The tooltip and menu list every device.
- Levels refresh every 30 seconds.

```sh
cargo install --path .
logi-battery-tray &
```

To start it at login, add it to GNOME Startup Applications or
`~/.config/autostart/`.

Check what UPower sees with `upower -d`.
A device missing there needs a direct HID++ backend over hidraw.

## macOS

Planned.
macOS has no kernel HID++ support, so it needs a direct HID++ backend
(`hidapi`, features 0x1000/0x1001/0x1004) and a signed app bundle for the
Input Monitoring permission.
