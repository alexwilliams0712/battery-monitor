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

/// The kernel refreshes HID++ battery levels slowly, so faster polling gains nothing.
const POLL: Duration = Duration::from_secs(30);

fn main() -> anyhow::Result<()> {
    // Hidden debug aid: on Linux it bypasses UPower.
    if std::env::args().any(|it| it == "--hidpp") {
        let batteries = runtime()?.block_on(hidpp::batteries())?;
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
    runtime()?.block_on(async {
        let system = zbus::Connection::system().await?;
        let session = zbus::Connection::session().await?;
        // At login the tray host may not be up yet.
        while let Err(e) = sni::spawn(&session).await {
            eprintln!("tray: {e}");
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let mut ticker = tokio::time::interval(POLL);
        loop {
            ticker.tick().await;
            let batteries = upower::peripherals(&system).await;
            if let Err(e) = async { sni::update(&session, batteries?).await }.await {
                eprintln!("update: {e}");
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
                loop {
                    ticker.tick().await;
                    let update = hidpp::batteries().await.map_err(|e| e.to_string());
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
