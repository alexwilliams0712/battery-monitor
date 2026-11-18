use crate::upower::Battery;
use std::collections::HashMap;
use zbus::{
    Connection, fdo, interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedValue, StructureBuilder, Type, Value},
};

const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";

/// Above this many devices the label drops names and shows only percentages.
const FULL_LABEL_MAX: usize = 4;

/// Text shown beside the tray icon.
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

fn line(battery: &Battery) -> String {
    let Battery {
        model,
        percentage,
        state,
        icon_name: _,
    } = battery;
    format!("{model}: {percentage:.0}% ({state})")
}

#[zbus::proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
trait Watcher {
    fn register_status_notifier_item(&self, service: &str) -> zbus::Result<()>;
}

/// Icon name, ARGB pixmaps, title and description.
type ToolTip = (String, Vec<(i32, i32, Vec<u8>)>, String, String);

/// `org.kde.StatusNotifierItem`, plus the Ayatana label extension.
// ksni does not expose `XAyatanaLabel`, which is the only way to put text
// in the Ubuntu top bar, so the item is implemented directly.
#[derive(Debug, Default)]
struct Item {
    batteries: Vec<Battery>,
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property)]
    fn category(&self) -> &str {
        "Hardware"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        env!("CARGO_PKG_NAME")
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        "Device batteries"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn icon_name(&self) -> String {
        self.batteries
            .iter()
            .min_by(|a, b| a.percentage.total_cmp(&b.percentage))
            .map_or("battery-missing-symbolic", |it| &it.icon_name)
            .into()
    }

    #[zbus(property)]
    fn tool_tip(&self) -> ToolTip {
        let description = self.batteries.iter().map(line).collect::<Vec<_>>();
        (
            String::new(),
            Vec::new(),
            "Device batteries".into(),
            description.join("\n"),
        )
    }

    #[zbus(property)]
    fn menu(&self) -> ObjectPath<'_> {
        ObjectPath::from_static_str_unchecked(MENU_PATH)
    }

    #[zbus(property)]
    fn item_is_menu(&self) -> bool {
        true
    }

    #[zbus(property, name = "XAyatanaLabel")]
    fn ayatana_label(&self) -> String {
        label(&self.batteries)
    }

    #[zbus(property, name = "XAyatanaLabelGuide")]
    fn ayatana_label_guide(&self) -> &str {
        ""
    }

    fn activate(&self, _x: i32, _y: i32) {}

    fn secondary_activate(&self, _x: i32, _y: i32) {}

    fn context_menu(&self, _x: i32, _y: i32) {}

    fn scroll(&self, _delta: i32, _orientation: &str) {}

    #[zbus(signal)]
    async fn new_icon(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn new_tool_tip(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal, name = "XAyatanaNewLabel")]
    async fn new_label(emitter: &SignalEmitter<'_>, label: &str, guide: &str) -> zbus::Result<()>;
}

/// `com.canonical.dbusmenu` with a flat menu:
/// one disabled row per device, a separator and Quit.
#[derive(Debug, Default)]
struct Menu {
    batteries: Vec<Battery>,
    revision: u32,
}

const ROOT_ID: i32 = 0;
const QUIT_ID: i32 = 1;
const SEPARATOR_ID: i32 = 2;
const FIRST_DEVICE_ID: i32 = 3;

type Props = HashMap<String, OwnedValue>;

fn owned(value: impl Into<Value<'static>>) -> OwnedValue {
    value
        .into()
        .try_into_owned()
        .expect("menu values MUST NOT contain file descriptors")
}

#[derive(Debug, Type, serde::Serialize)]
struct Layout {
    id: i32,
    properties: Props,
    children: Vec<OwnedValue>,
}

impl Menu {
    fn items(&self) -> Vec<(i32, Props)> {
        let devices = (FIRST_DEVICE_ID..).zip(&self.batteries).map(|(id, it)| {
            let props = Props::from([
                ("label".into(), owned(line(it))),
                ("icon-name".into(), owned(it.icon_name.clone())),
                ("enabled".into(), owned(false)),
            ]);
            (id, props)
        });
        let tail = [
            (
                SEPARATOR_ID,
                Props::from([("type".into(), owned("separator"))]),
            ),
            (
                QUIT_ID,
                Props::from([
                    ("label".into(), owned("Quit")),
                    ("icon-name".into(), owned("application-exit")),
                ]),
            ),
        ];
        devices.chain(tail).collect()
    }
}

#[interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn get_layout(
        &self,
        parent_id: i32,
        _recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> fdo::Result<(u32, Layout)> {
        if parent_id != ROOT_ID {
            return self
                .items()
                .into_iter()
                .find(|(id, _)| *id == parent_id)
                .map(|(id, properties)| {
                    let layout = Layout {
                        id,
                        properties,
                        children: Vec::new(),
                    };
                    (self.revision, layout)
                })
                .ok_or_else(|| fdo::Error::InvalidArgs("unknown menu id".into()));
        }
        let children = self
            .items()
            .into_iter()
            .map(|(id, properties)| {
                let structure = StructureBuilder::new()
                    .add_field(id)
                    .add_field(properties)
                    .add_field(Vec::<OwnedValue>::new())
                    .build()
                    .expect("menu item structure MUST have fields");
                owned(structure)
            })
            .collect();
        let root = Layout {
            id: ROOT_ID,
            properties: Props::from([("children-display".into(), owned("submenu"))]),
            children,
        };
        Ok((self.revision, root))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _property_names: Vec<String>,
    ) -> Vec<(i32, Props)> {
        self.items()
            .into_iter()
            .filter(|(id, _)| ids.is_empty() || ids.contains(id))
            .collect()
    }

    fn get_property(&self, id: i32, name: &str) -> fdo::Result<OwnedValue> {
        self.items()
            .into_iter()
            .find(|(it, _)| *it == id)
            .and_then(|(_, mut props)| props.remove(name))
            .ok_or_else(|| fdo::Error::InvalidArgs("unknown menu property".into()))
    }

    fn event(&self, id: i32, event_id: &str, _data: OwnedValue, _timestamp: u32) {
        if id == QUIT_ID && event_id == "clicked" {
            std::process::exit(0);
        }
    }

    fn event_group(&self, events: Vec<(i32, String, OwnedValue, u32)>) -> Vec<i32> {
        for (id, event_id, data, timestamp) in events {
            self.event(id, &event_id, data, timestamp);
        }
        Vec::new()
    }

    fn about_to_show(&self, _id: i32) -> bool {
        false
    }

    fn about_to_show_group(&self, _ids: Vec<i32>) -> (Vec<i32>, Vec<i32>) {
        (Vec::new(), Vec::new())
    }

    #[zbus(property)]
    fn version(&self) -> u32 {
        3
    }

    #[zbus(property)]
    fn text_direction(&self) -> &str {
        "ltr"
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "normal"
    }

    #[zbus(property)]
    fn icon_theme_path(&self) -> Vec<String> {
        Vec::new()
    }

    #[zbus(signal)]
    async fn layout_updated(
        emitter: &SignalEmitter<'_>,
        revision: u32,
        parent: i32,
    ) -> zbus::Result<()>;
}

/// Exports the item on the session bus and registers it with the tray host.
pub async fn spawn(conn: &Connection) -> zbus::Result<()> {
    let server = conn.object_server();
    server.at(ITEM_PATH, Item::default()).await?;
    server.at(MENU_PATH, Menu::default()).await?;
    let name = format!("org.kde.StatusNotifierItem-{}-1", std::process::id());
    conn.request_name(name.as_str()).await?;
    WatcherProxy::new(conn)
        .await?
        .register_status_notifier_item(&name)
        .await
}

pub async fn update(conn: &Connection, batteries: Vec<Battery>) -> zbus::Result<()> {
    let server = conn.object_server();
    let item = server.interface::<_, Item>(ITEM_PATH).await?;
    if item.get().await.batteries == batteries {
        return Ok(());
    }
    let text = label(&batteries);
    item.get_mut().await.batteries = batteries.clone();
    let emitter = item.signal_emitter();
    Item::new_icon(emitter).await?;
    Item::new_tool_tip(emitter).await?;
    Item::new_label(emitter, &text, "").await?;

    let menu = server.interface::<_, Menu>(MENU_PATH).await?;
    let revision = {
        let mut it = menu.get_mut().await;
        it.batteries = batteries;
        it.revision += 1;
        it.revision
    };
    Menu::layout_updated(menu.signal_emitter(), revision, ROOT_ID).await
}

#[test]
fn label_shows_names_up_to_four_devices() {
    use crate::upower::State;
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
