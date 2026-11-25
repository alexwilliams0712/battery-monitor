use crate::{
    battery::{self, Battery, State},
    hidpp::PairedDevice,
};
use futures_lite::{Stream, StreamExt as _};
use zbus::{Connection, MatchRule, MessageStream, message::Type, zvariant::OwnedObjectPath};

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
    fn serial(&self) -> zbus::Result<String>;
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

pub enum Change {
    Devices,
    Properties,
}

pub async fn changes(
    conn: &Connection,
) -> zbus::Result<impl Stream<Item = zbus::Result<Change>> + Unpin> {
    let rule = MatchRule::builder()
        .msg_type(Type::Signal)
        .sender("org.freedesktop.UPower")?
        .path_namespace("/org/freedesktop/UPower")?
        .build();
    let stream = MessageStream::for_match_rule(rule, conn, None).await?;
    Ok(stream.filter_map(|msg| match msg {
        Ok(msg) => match msg.header().member()?.as_str() {
            "DeviceAdded" | "DeviceRemoved" => Some(Ok(Change::Devices)),
            "PropertiesChanged" => Some(Ok(Change::Properties)),
            _ => None,
        },
        Err(e) => Some(Err(e)),
    }))
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
            device: battery::Device {
                model: device.model().await?,
                serial: unit_serial(&device.serial().await.unwrap_or_default()),
            },
            percentage: device.percentage().await?,
            state: device.state().await?.into(),
            icon_name: device.icon_name().await?,
        });
    }
    Ok(out)
}

/// The kernel formats HID++ unit serials as four hyphen-separated hex octets.
fn unit_serial(serial: &str) -> Option<u32> {
    if serial.len() != 11
        || !serial
            .split('-')
            .all(|part| part.len() == 2 && part.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return None;
    }
    u32::from_str_radix(&serial.replace('-', ""), 16)
        .ok()
        .filter(|&id| id != 0 && id != u32::MAX)
}

fn prefer_short_name(device: &mut battery::Device, candidate: &battery::Device) {
    let name = candidate.short_name();
    if !name.is_empty()
        && (device.short_name().is_empty() || name.len() < device.short_name().len())
    {
        device.model.clone_from(&candidate.model);
    }
}

/// Prefer UPower readings and the receiver's short device names.
// The kernel drives only the receivers it lists, e.g. not the Lightspeed `c545`,
// so devices paired with newer receivers reach UPower late or never.
pub fn merge(mut upower: Vec<Battery>, hidpp: &[PairedDevice]) -> Vec<Battery> {
    // UPower can itself report both the wired and wireless connections.
    upower.sort_by_key(|it| (it.device.serial, it.state != State::Charging));
    upower.dedup_by(|other, first| {
        if first.device.serial.is_none() || first.device.serial != other.device.serial {
            return false;
        }
        prefer_short_name(&mut first.device, &other.device);
        true
    });
    let mut out = Vec::new();
    for paired in hidpp {
        let matched = paired
            .device
            .serial
            .and_then(|id| upower.iter().position(|it| it.device.serial == Some(id)))
            .or_else(|| {
                upower
                    .iter()
                    .position(|it| it.device.matches(&paired.device))
            });
        if let Some(ix) = matched {
            let mut reading = upower.swap_remove(ix);
            prefer_short_name(&mut reading.device, &paired.device);
            out.push(reading);
            continue;
        }
        if let Some(mut reading) = paired.battery() {
            reading.icon_name = icon_name(reading.percentage, reading.state);
            out.push(reading);
        }
    }
    out.extend(upower);
    out.sort_by(|a, b| a.device.sort_key().cmp(&b.device.sort_key()));
    out
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
        device: battery::Device {
            model: model.into(),
            serial: None,
        },
        percentage,
        state: State::Discharging,
        icon_name: String::new(),
    };
    let merged = merge(
        vec![battery("Logitech G502", 10.0)],
        &[("Logitech G915 TKL", 93.0), ("Logitech G502", 50.0)].map(|(model, percentage)| {
            PairedDevice {
                device: battery::Device {
                    model: model.into(),
                    serial: None,
                },
                reading: Some((percentage, State::Discharging)),
            }
        }),
    );
    let got: Vec<_> = merged
        .iter()
        .map(|it| {
            (
                it.device.model.as_str(),
                it.percentage,
                it.icon_name.as_str(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("Logitech G502", 10.0, ""),
            ("Logitech G915 TKL", 93.0, "battery-level-90-symbolic"),
        ]
    );
}

#[test]
fn wired_and_wireless_readings_share_a_unit_serial() {
    let serial = unit_serial("01-23-ab-cd");
    assert_eq!(serial, Some(0x0123_abcd));
    let wired = Battery {
        device: battery::Device {
            model: "G915 TKL LIGHTSPEED Wireless RGB Mechanical Gaming Keyboard".into(),
            serial,
        },
        percentage: 63.0,
        state: State::Charging,
        icon_name: "battery-full-charging-symbolic".into(),
    };
    let keyboard = battery::Device {
        model: "Logitech G915 TKL".into(),
        serial,
    };
    let other = battery::Device {
        serial: Some(2),
        ..keyboard.clone()
    };
    let wireless = Battery {
        device: keyboard.clone(),
        percentage: 60.0,
        state: State::Discharging,
        icon_name: String::new(),
    };
    for reading in [None, Some((60.0, State::Discharging))] {
        let paired = [
            PairedDevice {
                device: keyboard.clone(),
                reading,
            },
            PairedDevice {
                device: other.clone(),
                reading: Some((25.0, State::Discharging)),
            },
        ];
        for upower in [vec![wired.clone()], vec![wireless.clone(), wired.clone()]] {
            let got = merge(upower, &paired);
            assert_eq!(battery::label(&got), "G915 TKL 25%  │  G915 TKL 63%");
            assert_eq!(
                got.iter().find(|it| it.device.serial == serial),
                Some(&Battery {
                    device: keyboard.clone(),
                    ..wired.clone()
                })
            );
        }
    }
}
