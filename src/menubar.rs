use crate::battery::{Battery, DIVIDER, State, line};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::{
    NSAttributedStringAttachmentConveniences as _, NSColor, NSImage, NSImageSymbolConfiguration,
    NSTextAttachment,
};
use objc2_foundation::{
    NSArray, NSAttributedString, NSMutableAttributedString, NSPoint, NSRect, NSSize, NSString,
};
use tao::{
    event::{Event, StartCause},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    platform::macos::{ActivationPolicy, EventLoopExtMacOS as _},
};
use tray_icon::{
    TrayIcon, TrayIconBuilder,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

const QUIT_ID: &str = "quit";
const APPEND: &str = "appending to a fresh menu MUST succeed";
/// Shown while there is nothing to report, so the item stays clickable.
const FALLBACK_TITLE: &str = "Batteries";

/// Batteries, or an error message to show in the menu.
pub type Update = Result<Vec<Battery>, String>;

/// Runs the AppKit event loop on the main thread; `spawn` starts the poller.
pub fn run(spawn: impl FnOnce(EventLoopProxy<Update>)) -> ! {
    let mut event_loop = EventLoopBuilder::<Update>::with_user_event().build();
    // A menu bar item only: no Dock icon or app menu.
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    spawn(event_loop.create_proxy());
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
    match update.as_deref() {
        Ok(batteries) if !batteries.is_empty() => {
            let main_thread =
                MainThreadMarker::new().expect("AppKit updates run on the main thread");
            if let Some(button) = tray
                .ns_status_item()
                .and_then(|item| item.button(main_thread))
            {
                button.setAttributedTitle(&title(batteries));
            }
        }
        _ => tray.set_title(Some(FALLBACK_TITLE)),
    }
    tray.set_menu(Some(Box::new(menu(update))));
}

fn title(batteries: &[Battery]) -> Retained<NSMutableAttributedString> {
    let text = NSMutableAttributedString::new();
    let append = |s: &str| {
        text.appendAttributedString(&NSAttributedString::from_nsstring(&NSString::from_str(s)))
    };
    for (ix, battery) in batteries.iter().enumerate() {
        if ix > 0 {
            append(DIVIDER);
        }
        match battery_icon(battery) {
            Some(icon) => text.appendAttributedString(&icon),
            None if battery.state == State::Charging => append("⚡︎▯"),
            None => append("▯"),
        }
        append(&format!(" {}", battery.label(batteries.len())));
    }
    text
}

fn battery_icon(battery: &Battery) -> Option<Retained<NSAttributedString>> {
    if !objc2::available!(macos = 11.0) {
        return None;
    }
    let charging = battery.state == State::Charging;
    let symbol = match (charging, battery.percentage.round() as u8) {
        (true, _) => "battery.100.bolt",
        (_, 0..=12) => "battery.0",
        (_, 13..=37) => "battery.25",
        (_, 38..=62) => "battery.50",
        (_, 63..=87) => "battery.75",
        _ => "battery.100",
    };
    let mut image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(symbol),
        Some(&NSString::from_str(&battery.state.to_string())),
    )?;
    if charging && objc2::available!(macos = 12.0) {
        let colours = NSArray::from_retained_slice(&[NSColor::systemGreenColor()]);
        let config = NSImageSymbolConfiguration::configurationWithPaletteColors(&colours);
        image = image.imageWithSymbolConfiguration(&config).unwrap_or(image);
    }
    let attachment = NSTextAttachment::new();
    attachment.setImage(Some(&image));
    attachment.setBounds(NSRect::new(
        NSPoint::new(0.0, -2.0),
        NSSize::new(18.0, 12.0),
    ));
    Some(NSAttributedString::attributedStringWithAttachment(
        &attachment,
    ))
}

fn menu(update: &Update) -> Menu {
    let rows = match update {
        Ok(batteries) if batteries.is_empty() => vec!["No devices found".into()],
        Ok(batteries) => batteries.iter().map(line).collect(),
        Err(e) => vec![format!("Error: {e}")],
    };
    let menu = Menu::new();
    for row in &rows {
        menu.append(&MenuItem::new(row, false, None)).expect(APPEND);
    }
    let quit = MenuItem::with_id(QUIT_ID, "Quit", true, None);
    menu.append_items(&[&PredefinedMenuItem::separator(), &quit])
        .expect(APPEND);
    menu
}
