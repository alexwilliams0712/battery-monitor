use crate::upower::Battery;
use ksni::{MenuItem, ToolTip, menu::StandardItem};

/// StatusNotifierItem showing the lowest battery as the icon
/// and every battery in the tooltip and menu.
#[derive(Debug, Default)]
pub struct Tray {
    pub batteries: Vec<Battery>,
}

impl Tray {
    fn lowest(&self) -> Option<&Battery> {
        self.batteries
            .iter()
            .min_by(|a, b| a.percentage.total_cmp(&b.percentage))
    }
}

fn line(battery: &Battery) -> String {
    let Battery {
        model,
        percentage,
        state,
        icon_name: _,
    } = battery;
    format!("{model}: {percentage:.0}% ({state})")
}

impl ksni::Tray for Tray {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }

    fn title(&self) -> String {
        "Device batteries".into()
    }

    fn icon_name(&self) -> String {
        self.lowest()
            .map_or("battery-missing-symbolic", |it| &it.icon_name)
            .into()
    }

    fn tool_tip(&self) -> ToolTip {
        let description = match self.batteries.is_empty() {
            true => "No wireless devices found".into(),
            false => self
                .batteries
                .iter()
                .map(line)
                .collect::<Vec<_>>()
                .join("\n"),
        };
        ToolTip {
            title: self.title(),
            description,
            ..Default::default()
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = self
            .batteries
            .iter()
            .map(|it| {
                StandardItem {
                    label: line(it),
                    icon_name: it.icon_name.clone(),
                    enabled: false,
                    ..Default::default()
                }
                .into()
            })
            .collect();
        items.extend([
            MenuItem::Separator,
            StandardItem {
                label: "Quit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]);
        items
    }
}
