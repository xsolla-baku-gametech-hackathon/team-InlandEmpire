//! `tappad-bridge --fake` or `tappad-bridge --port /dev/cu.usbserial-XXXX`.

use std::time::Duration;

use clap::Parser;
use tappad_bridge::{serial, source, ws};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

#[derive(Parser)]
#[command(about = "Forwards pad taps to the game over WebSocket")]
struct Args {
    /// Invent a tap every few seconds instead of reading a pad.
    #[arg(long, conflicts_with = "port", required_unless_present = "port")]
    fake: bool,
    /// Serial port of the pad, for example /dev/cu.usbserial-0001 or COM3.
    #[arg(long)]
    port: Option<String>,
    /// Seconds between fake taps.
    #[arg(long, default_value_t = 5)]
    interval: u64,
    /// Address the game connects to.
    #[arg(long, default_value = tappad_protocol::BRIDGE_WS_ADDR)]
    addr: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env().add_directive("info".parse()?),
        )
        .init();
    let args = Args::parse();
    let (tx, _) = broadcast::channel(64);
    let listener = TcpListener::bind(&args.addr).await?;
    tracing::info!(addr = %args.addr, "tappad-bridge listening");
    tokio::spawn(ws::serve(listener, tx.clone()));
    if let Some(port) = args.port {
        tokio::task::spawn_blocking(move || serial::run_serial(&port, &tx)).await?
    } else {
        source::run_fake(tx, Duration::from_secs(args.interval)).await
    }
}
