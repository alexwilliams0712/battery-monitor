use core::fmt;

/// Above this many devices the label drops names and shows only percentages.
const FULL_LABEL_MAX: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct Battery {
    pub model: String,
    pub percentage: f64,
    pub state: State,
    /// Freedesktop icon name for this level and state, empty when the backend has none.
    pub icon_name: String,
}

impl Battery {
    /// Model without the vendor prefix, e.g. `G502`.
    pub fn short_name(&self) -> &str {
        self.model.strip_prefix("Logitech ").unwrap_or(&self.model)
    }
}

/// Charging state, modelled on `org.freedesktop.UPower.Device.State`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    not(target_os = "linux"),
    expect(dead_code, reason = "only UPower reports some states")
)]
pub enum State {
    Unknown,
    Charging,
    Discharging,
    Empty,
    FullyCharged,
    PendingCharge,
    PendingDischarge,
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

/// Text shown in the top bar or menu bar.
pub fn label(batteries: &[Battery]) -> String {
    let compact = batteries.len() > FULL_LABEL_MAX;
    batteries
        .iter()
        .map(|it| match compact {
            true => format!("{:.0}%", it.percentage),
            false => format!("{} {:.0}%", it.short_name(), it.percentage),
        })
        .collect::<Vec<_>>()
        .join("  ")
}

#[test]
fn label_shows_names_up_to_four_devices() {
    let battery = |model: &str, percentage| Battery {
        model: model.into(),
        percentage,
        state: State::Discharging,
        icon_name: String::new(),
    };
    let two = [
        battery("Logitech G502", 12.0),
        battery("Logitech G915 TKL", 37.4),
    ];
    assert_eq!(label(&two), "G502 12%  G915 TKL 37%");
    let four = [0.0, 1.0, 2.0, 3.0].map(|it| battery("Pad", it));
    assert_eq!(label(&four), "Pad 0%  Pad 1%  Pad 2%  Pad 3%");
    let five = [10.0, 20.0, 30.0, 40.0, 50.0].map(|it| battery("Pad", it));
    assert_eq!(label(&five), "10%  20%  30%  40%  50%");
}

/// One menu row, e.g. `Logitech G502: 12% (discharging)`.
pub fn line(battery: &Battery) -> String {
    let Battery {
        model,
        percentage,
        state,
        icon_name: _,
    } = battery;
    format!("{model}: {percentage:.0}% ({state})")
}
