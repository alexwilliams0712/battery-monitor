use core::fmt;

/// Above this many devices the label drops the device names.
const FULL_LABEL_MAX: usize = 4;
pub const DIVIDER: &str = "  │  ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Device {
    pub model: String,
    /// HID++ unit serial shared by the receiver and the wired connection.
    pub serial: Option<u32>,
}

impl Device {
    pub fn matches(&self, other: &Self) -> bool {
        match (self.serial, other.serial) {
            (Some(a), Some(b)) => a == b,
            _ => self.short_name() == other.short_name(),
        }
    }

    pub fn sort_key(&self) -> (&str, Option<u32>) {
        (self.short_name(), self.serial)
    }

    pub fn short_name(&self) -> &str {
        self.model.strip_prefix("Logitech ").unwrap_or(&self.model)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub device: Device,
    pub percentage: f64,
    pub state: State,
    /// Freedesktop icon name for this level and state, empty when the backend has none.
    pub icon_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Unknown,
    Charging,
    Discharging,
    Full,
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unknown => "unknown",
            Self::Charging => "charging",
            Self::Discharging => "discharging",
            Self::Full => "full",
        })
    }
}

impl Battery {
    pub fn label(&self, device_count: usize) -> String {
        match device_count > FULL_LABEL_MAX {
            true => format!("{:.0}%", self.percentage),
            false => format!("{} {:.0}%", self.device.short_name(), self.percentage),
        }
    }
}

/// Plain-text equivalent of the panel for diagnostics.
pub fn label(batteries: &[Battery]) -> String {
    batteries
        .iter()
        .map(|it| it.label(batteries.len()))
        .collect::<Vec<_>>()
        .join(DIVIDER)
}

#[test]
fn label_shows_names_up_to_four_devices() {
    let battery = |model: &str, percentage| Battery {
        device: Device {
            model: model.into(),
            serial: None,
        },
        percentage,
        state: State::Discharging,
        icon_name: String::new(),
    };
    let two = [
        battery("Logitech G502", 12.0),
        battery("Logitech G915 TKL", 37.4),
    ];
    assert_eq!(label(&two), "G502 12%  │  G915 TKL 37%");
    let four = [0.0, 1.0, 2.0, 3.0].map(|it| battery("Pad", it));
    assert_eq!(label(&four), "Pad 0%  │  Pad 1%  │  Pad 2%  │  Pad 3%");
    let five = [10.0, 20.0, 30.0, 40.0, 50.0].map(|it| battery("Pad", it));
    assert_eq!(label(&five), "10%  │  20%  │  30%  │  40%  │  50%");
}

/// One menu row, e.g. `G502: 12% (discharging)`.
pub fn line(battery: &Battery) -> String {
    let Battery {
        device,
        percentage,
        state,
        icon_name: _,
    } = battery;
    format!("{}: {percentage:.0}% ({state})", device.short_name())
}
