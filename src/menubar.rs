use crate::battery::{Battery, label, line};
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    platform::macos::{ActivationPolicy, EventLoopExtMacOS as _},
};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{IsMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const QUIT_ID: &str = "quit";
/// Shown while there is nothing to report, so the item stays clickable.
const FALLBACK_TITLE: &str = "Batteries";

/// Batteries, or an error message to show in the menu.
pub type Update = Result<Vec<Battery>, String>;

/// Hands updates from the polling thread to the main thread.
pub struct Updater(EventLoopProxy<Update>);

impl Updater {
    pub fn send(&self, update: Update) {
        let Self(proxy) = self;
        // Fails only once the event loop has exited, i.e. the app is quitting.
        let _ = proxy.send_event(update);
    }
}

/// Runs the AppKit event loop on the main thread; `spawn` starts the poller.
pub fn run(spawn: impl FnOnce(Updater)) -> ! {
    let mut event_loop = EventLoopBuilder::<Update>::with_user_event().build();
    // A menu bar item only: no Dock icon or app menu.
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    spawn(Updater(event_loop.create_proxy()));
    MenuEvent::set_event_handler(Some(|event: MenuEvent| {
        if event.id == QUIT_ID {
            std::process::exit(0);
        }
    }));
    let mut tray: Option<TrayIcon> = None;
    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            // AppKit MUST be running before the status item is created.
            Event::NewEvents(StartCause::Init) => {
                let built = TrayIconBuilder::new()
                    .with_title(FALLBACK_TITLE)
                    .with_menu(Box::new(menu(&Ok(Vec::new()))))
                    .build();
                match built {
                    Ok(it) => tray = Some(it),
                    Err(e) => eprintln!("tray: {e}"),
                }
            }
            Event::UserEvent(update) => {
                if let Some(tray) = &tray {
                    show(tray, &update);
                }
            }
            _ => {}
        }
    })
}

fn show(tray: &TrayIcon, update: &Update) {
    let text = update.as_deref().map(label).unwrap_or_default();
    match text.is_empty() {
        true => tray.set_title(Some(FALLBACK_TITLE)),
        false => tray.set_title(Some(text)),
    }
    tray.set_menu(Some(Box::new(menu(update))));
}

/// One disabled row per device or the error, a separator and Quit.
fn menu(update: &Update) -> Menu {
    let rows = match update {
        Ok(batteries) if batteries.is_empty() => vec!["No devices found".into()],
        Ok(batteries) => batteries.iter().map(line).collect(),
        Err(e) => vec![format!("Error: {e}")],
    };
    let rows = rows
        .iter()
        .map(|it| MenuItem::new(it, false, None))
        .collect::<Vec<_>>();
    let separator = PredefinedMenuItem::separator();
    let quit = MenuItem::with_id(QUIT_ID, "Quit", true, None);
    let menu = Menu::new();
    rows.iter()
        .map(|it| it as &dyn IsMenuItem)
        .chain([&separator as &dyn IsMenuItem, &quit])
        .for_each(|it| {
            menu.append(it)
                .expect("appending to a fresh menu MUST succeed");
        });
    menu
}
