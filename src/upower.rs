use crate::battery::{Battery, State};
use zbus::{Connection, zvariant::OwnedObjectPath};

#[zbus::proxy(
    interface = "org.freedesktop.UPower",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower"
)]
trait UPower {
    fn enumerate_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

#[zbus::proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower"
)]
trait Device {
    #[zbus(property)]
    fn model(&self) -> zbus::Result<String>;
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn power_supply(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;
    #[zbus(property)]
    fn icon_name(&self) -> zbus::Result<String>;
}

/// `org.freedesktop.UPower.Device.State`.
impl From<u32> for State {
    fn from(raw: u32) -> Self {
        match raw {
            1 => Self::Charging,
            2 | 3 => Self::Discharging,
            4 => Self::Full,
            _ => Self::Unknown,
        }
    }
}

/// Present batteries that do not power the host, i.e. peripherals.
// The kernel `hid-logitech-hidpp` driver speaks HID++ to Logitech receivers
// and exports each paired device as a power supply, which UPower republishes.
pub async fn peripherals(conn: &Connection) -> zbus::Result<Vec<Battery>> {
    let mut out = Vec::new();
    for path in UPowerProxy::new(conn).await?.enumerate_devices().await? {
        let device = DeviceProxy::builder(conn).path(path)?.build().await?;
        if device.power_supply().await? || !device.is_present().await? {
            continue;
        }
        out.push(Battery {
            model: device.model().await?,
            percentage: device.percentage().await?,
            state: device.state().await?.into(),
            icon_name: device.icon_name().await?,
        });
    }
    out.sort_by(|a, b| a.model.cmp(&b.model));
    Ok(out)
}

/// `upower` plus HID++ readings for devices it lacks, matched by model.
// The kernel drives only the receivers it lists, e.g. not the Lightspeed `c545`,
// so devices paired with newer receivers reach UPower late or never.
pub fn merge(mut upower: Vec<Battery>, hidpp: &[Battery]) -> Vec<Battery> {
    let extra: Vec<_> = hidpp
        .iter()
        .filter(|it| !upower.iter().any(|known| known.model == it.model))
        .map(|it| Battery {
            icon_name: icon_name(it.percentage, it.state),
            ..it.clone()
        })
        .collect();
    upower.extend(extra);
    upower.sort_by(|a, b| a.model.cmp(&b.model));
    upower
}

/// The icon UPower would give this level, e.g. `battery-level-30-charging-symbolic`.
fn icon_name(percentage: f64, state: State) -> String {
    let level = (percentage / 10.0).round() * 10.0;
    let suffix = match state {
        State::Charging => "-charging",
        State::Full => "-charged",
        State::Unknown | State::Discharging => "",
    };
    format!("battery-level-{level:.0}{suffix}-symbolic")
}

#[test]
fn merge_prefers_upower() {
    let battery = |model: &str, percentage| Battery {
        model: model.into(),
        percentage,
        state: State::Discharging,
        icon_name: String::new(),
    };
    let merged = merge(
        vec![battery("Logitech G502", 10.0)],
        &[
            battery("Logitech G915 TKL", 93.0),
            battery("Logitech G502", 50.0),
        ],
    );
    let got: Vec<_> = merged
        .iter()
        .map(|it| (it.model.as_str(), it.percentage, it.icon_name.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("Logitech G502", 10.0, ""),
            ("Logitech G915 TKL", 93.0, "battery-level-90-symbolic"),
        ]
    );
}
