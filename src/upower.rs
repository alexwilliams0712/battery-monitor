use core::fmt;
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

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub model: String,
    pub percentage: f64,
    pub state: State,
    /// Freedesktop icon name chosen by UPower for this level and state.
    pub icon_name: String,
}

/// `org.freedesktop.UPower.Device.State`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Unknown,
    Charging,
    Discharging,
    Empty,
    FullyCharged,
    PendingCharge,
    PendingDischarge,
}

impl From<u32> for State {
    fn from(raw: u32) -> Self {
        match raw {
            1 => Self::Charging,
            2 => Self::Discharging,
            3 => Self::Empty,
            4 => Self::FullyCharged,
            5 => Self::PendingCharge,
            6 => Self::PendingDischarge,
            _ => Self::Unknown,
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unknown => "unknown",
            Self::Charging => "charging",
            Self::Discharging => "discharging",
            Self::Empty => "empty",
            Self::FullyCharged => "full",
            Self::PendingCharge => "pending charge",
            Self::PendingDischarge => "pending discharge",
        })
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
