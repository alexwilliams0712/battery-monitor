mod sni;
mod upower;

use core::time::Duration;

/// The kernel refreshes HID++ battery levels slowly, so faster polling gains nothing.
const POLL: Duration = Duration::from_secs(30);

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
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
        let result = match upower::peripherals(&system).await {
            Ok(batteries) => sni::update(&session, batteries).await,
            Err(e) => Err(e),
        };
        if let Err(e) = result {
            eprintln!("update: {e}");
        }
    }
}
