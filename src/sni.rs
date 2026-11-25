use crate::battery::{Battery, DIVIDER, State, line};
use std::{collections::HashMap, sync::Arc};
use zbus::{
    Connection, fdo, interface,
    object_server::SignalEmitter,
    zvariant::{ObjectPath, OwnedValue, Type, Value},
};

const ITEM_PATH: &str = "/StatusNotifierItem";
const MENU_PATH: &str = "/MenuBar";

#[zbus::proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
trait Watcher {
    fn register_status_notifier_item(&self, service: &str) -> zbus::Result<()>;
}

/// `org.kde.StatusNotifierItem`, plus the Ayatana label extension.
// ksni does not expose `XAyatanaLabel`, which is the only way to put text
// in the Ubuntu top bar, so the item is implemented directly.
#[derive(Debug, PartialEq, Eq)]
struct Item {
    id: String,
    label: String,
    icon: String,
    description: String,
}

impl Item {
    fn for_slot(slot: usize, batteries: &[Battery]) -> Self {
        // Ubuntu inserts new indicators on the left. Slot zero stays on the right.
        let battery = batteries.iter().rev().nth(slot);
        let mut label = battery
            .map(|it| it.label(batteries.len()))
            .unwrap_or_default();
        if slot > 0 {
            label.push_str(DIVIDER.trim_end());
        }
        let icon = match battery {
            Some(it) if it.state == State::Charging && !it.icon_name.contains("-charging") => {
                "battery-good-charging-symbolic"
            }
            Some(it) if !it.icon_name.is_empty() => &it.icon_name,
            _ => "battery-missing-symbolic",
        };
        Self {
            id: format!("{}-{}", env!("CARGO_PKG_NAME"), slot + 1),
            label,
            icon: icon.into(),
            description: battery
                .map(line)
                .unwrap_or_else(|| "Device batteries".into()),
        }
    }
}

#[test]
fn switching_the_cable_changes_only_the_device_icons() {
    use crate::battery::Device;
    let devices = ["G502", "G915 TKL"].map(|name| Battery {
        device: Device {
            model: name.into(),
            serial: None,
        },
        percentage: 65.0,
        state: State::Discharging,
        icon_name: "battery-good-symbolic".into(),
    });
    for charging in [[true, false], [false, true], [true, true], [false, false]] {
        let mut readings = devices.clone();
        for (device, charges) in readings.iter_mut().zip(charging) {
            if charges {
                device.state = State::Charging;
                device.icon_name = "battery-good-charging-symbolic".into();
            }
        }
        let entries = [Item::for_slot(1, &readings), Item::for_slot(0, &readings)];
        assert_eq!(
            [&*entries[0].label, &*entries[1].label],
            ["G502 65%  │", "G915 TKL 65%"]
        );
        assert_eq!(entries.map(|it| it.icon.contains("-charging")), charging);
    }
}

#[interface(name = "org.kde.StatusNotifierItem")]
impl Item {
    #[zbus(property)]
    fn category(&self) -> &str {
        "Hardware"
    }

    #[zbus(property)]
    fn id(&self) -> &str {
        &self.id
    }

    #[zbus(property)]
    fn title(&self) -> &str {
        &self.description
    }

    #[zbus(property)]
    fn status(&self) -> &str {
        "Active"
    }

    #[zbus(property)]
    fn icon_name(&self) -> &str {
        &self.icon
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
    fn ayatana_label(&self) -> &str {
        &self.label
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
    async fn new_title(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal, name = "XAyatanaNewLabel")]
    async fn new_label(emitter: &SignalEmitter<'_>, label: &str, guide: &str) -> zbus::Result<()>;
}

/// `com.canonical.dbusmenu` with a flat menu:
/// one disabled row per device, a separator and Quit.
#[derive(Debug, Default)]
struct Menu {
    batteries: Arc<[Battery]>,
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

#[derive(Debug, Type, Value, serde::Serialize)]
struct Layout {
    id: i32,
    properties: Props,
    children: Vec<OwnedValue>,
}

impl Layout {
    fn leaf((id, properties): (i32, Props)) -> Self {
        Self {
            id,
            properties,
            children: Vec::new(),
        }
    }
}

impl Menu {
    fn items(&self) -> impl Iterator<Item = (i32, Props)> + '_ {
        let devices = (FIRST_DEVICE_ID..)
            .zip(self.batteries.iter())
            .map(|(id, it)| {
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
        devices.chain(tail)
    }
}

#[interface(name = "com.canonical.dbusmenu")]
impl Menu {
    fn get_layout(
        &self,
        parent_id: i32,
        recursion_depth: i32,
        _property_names: Vec<String>,
    ) -> fdo::Result<(u32, Layout)> {
        let mut items = self.items();
        let layout = match parent_id {
            ROOT_ID => Layout {
                id: ROOT_ID,
                properties: Props::from([("children-display".into(), owned("submenu"))]),
                children: match recursion_depth {
                    0 => Vec::new(),
                    _ => items.map(|it| owned(Layout::leaf(it))).collect(),
                },
            },
            _ => items
                .find(|(id, _)| *id == parent_id)
                .map(Layout::leaf)
                .ok_or_else(|| fdo::Error::InvalidArgs("unknown menu id".into()))?,
        };
        Ok((self.revision, layout))
    }

    fn get_group_properties(
        &self,
        ids: Vec<i32>,
        _property_names: Vec<String>,
    ) -> Vec<(i32, Props)> {
        self.items()
            .filter(|(id, _)| ids.is_empty() || ids.contains(id))
            .collect()
    }

    fn get_property(&self, id: i32, name: &str) -> fdo::Result<OwnedValue> {
        self.items()
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

/// Each native indicator has its own icon and label, and opens the same device list.
pub struct Tray {
    slots: Vec<Connection>,
    displayed: Option<Arc<[Battery]>>,
}

impl Tray {
    pub async fn new(session: &Connection) -> zbus::Result<Self> {
        register(session, 0, &[]).await?;
        Ok(Self {
            slots: vec![session.clone()],
            displayed: None,
        })
    }

    pub async fn update(&mut self, batteries: Vec<Battery>) -> zbus::Result<()> {
        if self.displayed.as_deref() == Some(&batteries) {
            return Ok(());
        }
        self.displayed = None;
        let batteries: Arc<[Battery]> = batteries.into();
        let count = batteries.len().max(1);
        while self.slots.len() < count {
            let conn = Connection::session().await?;
            register(&conn, self.slots.len(), &batteries).await?;
            self.slots.push(conn);
        }
        for conn in self.slots.drain(count..) {
            conn.close().await?;
        }
        for (slot, conn) in self.slots.iter().enumerate() {
            let server = conn.object_server();
            let item = server.interface::<_, Item>(ITEM_PATH).await?;
            let entry = Item::for_slot(slot, &batteries);
            let emitter = item.signal_emitter();
            let label = entry.label.clone();
            *item.get_mut().await = entry;
            Item::new_icon(emitter).await?;
            Item::new_label(emitter, &label, "").await?;
            Item::new_title(emitter).await?;

            let menu = server.interface::<_, Menu>(MENU_PATH).await?;
            let revision = {
                let mut it = menu.get_mut().await;
                it.batteries = Arc::clone(&batteries);
                it.revision += 1;
                it.revision
            };
            Menu::layout_updated(menu.signal_emitter(), revision, ROOT_ID).await?;
        }
        self.displayed = Some(batteries);
        Ok(())
    }

    pub async fn register_with_host(&self) -> zbus::Result<()> {
        for (slot, conn) in self.slots.iter().enumerate() {
            register_with_host(conn, slot).await?;
        }
        Ok(())
    }
}

pub async fn host_changes(conn: &Connection) -> zbus::Result<zbus::proxy::OwnerChangedStream<'_>> {
    WatcherProxy::new(conn)
        .await?
        .inner()
        .receive_owner_changed()
        .await
}

fn service_name(slot: usize) -> String {
    format!(
        "org.kde.StatusNotifierItem-{}-{}",
        std::process::id(),
        slot + 1
    )
}

async fn register_with_host(conn: &Connection, slot: usize) -> zbus::Result<()> {
    WatcherProxy::new(conn)
        .await?
        .register_status_notifier_item(&service_name(slot))
        .await
}

async fn register(conn: &Connection, slot: usize, batteries: &[Battery]) -> zbus::Result<()> {
    let server = conn.object_server();
    server
        .at(ITEM_PATH, Item::for_slot(slot, batteries))
        .await?;
    server.at(MENU_PATH, Menu::default()).await?;
    conn.request_name(service_name(slot)).await?;
    register_with_host(conn, slot).await
}
