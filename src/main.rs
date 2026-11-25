mod battery;
mod hidpp;
#[cfg(target_os = "macos")]
mod menubar;
#[cfg(target_os = "linux")]
mod sni;
#[cfg(target_os = "linux")]
mod upower;

use core::time::Duration;
use tokio::runtime::{Builder, Runtime};

/// Background refresh for receivers that do not report changes through UPower.
const POLL: Duration = Duration::from_secs(30);

fn main() -> anyhow::Result<()> {
    // Hidden debug aid: HID++ only, without UPower or the cache.
    if std::env::args().any(|it| it == "--hidpp") {
        let devices = runtime()?.block_on(hidpp::devices(&[]))?;
        let batteries: Vec<_> = devices
            .iter()
            .filter_map(hidpp::PairedDevice::battery)
            .collect();
        println!("{}", battery::label(&batteries));
        batteries
            .iter()
            .for_each(|it| println!("{}", battery::line(it)));
        return Ok(());
    }
    run()
}

fn runtime() -> std::io::Result<Runtime> {
    Builder::new_current_thread().enable_all().build()
}

#[cfg(target_os = "linux")]
fn run() -> anyhow::Result<()> {
    use futures_lite::{StreamExt as _, future};
    use tokio::sync::Notify;

    runtime()?.block_on(async {
        let system = zbus::Connection::system().await?;
        let session = zbus::Connection::session().await?;
        let mut hosts = sni::host_changes(&session).await?;
        // At login the tray host may not be up yet.
        let mut tray = loop {
            match sni::Tray::new(&session).await {
                Ok(tray) => break tray,
                Err(e) => eprintln!("tray: {e}"),
            }
            tokio::time::sleep(Duration::from_secs(2)).await;
        };
        let mut changes = upower::changes(&system).await?;
        let rescan = Notify::new();
        rescan.notify_one();
        let mut hidpp = Vec::new();
        loop {
            let result = {
                let scan = async {
                    future::or(tokio::time::sleep(POLL), rescan.notified()).await;
                    hidpp::devices(&hidpp).await
                };
                tokio::pin!(scan);
                loop {
                    let update = async {
                        let batteries = upower::peripherals(&system).await?;
                        tray.update(upower::merge(batteries, &hidpp)).await
                    };
                    if let Err(e) = update.await {
                        eprintln!("update: {e}");
                    }
                    tokio::select! {
                        result = &mut scan => break result,
                        Some(owner) = hosts.next() => {
                            if owner.is_some() && let Err(e) = tray.register_with_host().await {
                                eprintln!("tray: {e}");
                            }
                        }
                        Some(change) = changes.next() => match change {
                            Ok(upower::Change::Devices) => rescan.notify_one(),
                            Ok(upower::Change::Properties) => {}
                            Err(e) => eprintln!("upower: {e}"),
                        },
                    }
                }
            };
            match result {
                Ok(it) => hidpp = it,
                Err(e) => eprintln!("hidpp: {e}"),
            }
        }
    })
}

/// AppKit owns the main thread, so HID++ polling runs on another.
#[cfg(target_os = "macos")]
fn run() -> anyhow::Result<()> {
    let runtime = runtime()?;
    menubar::run(move |proxy| {
        std::thread::spawn(move || {
            runtime.block_on(async {
                let mut ticker = tokio::time::interval(POLL);
                let mut last = Vec::new();
                loop {
                    ticker.tick().await;
                    let update = hidpp::devices(&last)
                        .await
                        .map(|devices| {
                            last = devices;
                            last.iter()
                                .filter_map(hidpp::PairedDevice::battery)
                                .collect()
                        })
                        .map_err(|e| e.to_string());
                    if let Err(e) = &update {
                        eprintln!("update: {e}");
                    }
                    // Fails only once the event loop has exited, i.e. while quitting.
                    let _ = proxy.send_event(update);
                }
            })
        });
    })
}
