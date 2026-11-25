# battery-monitor

Shows wireless peripheral battery levels in the Ubuntu top bar or macOS menu bar.
Each device has its own small battery icon. A green battery with a lightning bolt
shows which device is charging. Devices stay in name order, separated by a pipe,
e.g. `G502 12%  │  G915 TKL 37%`. With more than four devices the bar drops the names.
Click it to see each device with its charging state.

## Ubuntu

Works with Ubuntu 22.04 or newer on a 64-bit Intel or AMD PC.
It shows every peripheral battery Ubuntu knows about,
plus devices on Logitech receivers too new for Ubuntu to support.

1. Open the [latest release](https://github.com/alexwilliams0712/battery-monitor/releases/latest)
   and download the file ending in `_amd64.deb`.
2. Open Terminal (press Ctrl+Alt+T) and run:

   ```sh
   sudo apt install ~/Downloads/battery-monitor_*_amd64.deb
   setsid -f battery-monitor
   ```

3. Your battery levels appear at the top right of the screen.
   It starts by itself every time you log in.

To uninstall, run `sudo apt remove battery-monitor`.

## macOS

Works with Logitech devices that use a Unifying, Lightspeed or Bolt USB receiver.
Devices paired over Bluetooth are not shown.

1. Open Terminal (press Cmd+Space, type `Terminal`, press Return).
2. Install Apple's developer tools, then click Install in the window that opens:

   ```sh
   xcode-select --install
   ```

3. Install Rust. Press Return to accept the defaults:

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source "$HOME/.cargo/env"
   ```

4. Build and install battery-monitor. This takes a few minutes:

   ```sh
   cargo install --git https://github.com/alexwilliams0712/battery-monitor
   ```

5. Start it now and at every login:

   ```sh
   mkdir -p ~/Library/LaunchAgents
   cat > ~/Library/LaunchAgents/com.alexwilliams0712.battery-monitor.plist <<EOF
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0"><dict>
   <key>Label</key><string>com.alexwilliams0712.battery-monitor</string>
   <key>ProgramArguments</key><array><string>$HOME/.cargo/bin/battery-monitor</string></array>
   <key>RunAtLoad</key><true/>
   </dict></plist>
   EOF
   launchctl bootstrap "gui/$(id -u)" ~/Library/LaunchAgents/com.alexwilliams0712.battery-monitor.plist
   ```

6. Your battery levels appear in the menu bar at the top right.
   If it shows `Batteries` or an error instead, open System Settings >
   Privacy & Security > Input Monitoring, turn on `battery-monitor`,
   then log out and back in.

To uninstall, run:

```sh
launchctl bootout "gui/$(id -u)/com.alexwilliams0712.battery-monitor"
rm ~/Library/LaunchAgents/com.alexwilliams0712.battery-monitor.plist
cargo uninstall battery-monitor
```

## Releasing

Bump `version` in `Cargo.toml`, then push a matching `v*` tag.

## Licence

[PolyForm Strict 1.0.0](LICENSE.md).
