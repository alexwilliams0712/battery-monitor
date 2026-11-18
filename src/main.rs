mod tray;
mod upower;

use core::time::Duration;
use ksni::TrayMethods as _;

/// The kernel refreshes HID++ battery levels slowly, so faster polling gains nothing.
const POLL: Duration = Duration::from_secs(30);

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let conn = zbus::Connection::system().await?;
    let handle = tray::Tray::default().spawn().await?;
    let mut ticker = tokio::time::interval(POLL);
    loop {
        ticker.tick().await;
        match upower::peripherals(&conn).await {
            Ok(batteries) => {
                handle.update(|it| it.batteries = batteries).await;
            }
            Err(e) => eprintln!("upower: {e}"),
        }
    }
}
