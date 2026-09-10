//! The real pad: one JSON line per event over USB serial.

use std::io::{BufRead, BufReader};
use std::time::Duration;

use tappad_protocol::{parse_line, LineError};
use tokio::sync::broadcast;

use crate::source::to_line;

/// Baud rate the firmware uses.
pub const BAUD: u32 = 115_200;

/// Opens `port` and forwards every well-formed line until the port goes away.
/// Blocking; run it on its own thread.
///
/// # Errors
/// The port cannot be opened, or reading it fails.
pub fn run_serial(port: &str, tx: &broadcast::Sender<String>) -> anyhow::Result<()> {
    let reader = serialport::new(port, BAUD)
        .timeout(Duration::from_secs(3600))
        .open()?;
    tracing::info!(port, "serial open");
    for line in BufReader::new(reader).lines() {
        let line = line?;
        match parse_line(&line) {
            Ok(event) => {
                tracing::info!(?event, "pad");
                let _ = tx.send(to_line(&event)?);
            }
            Err(LineError::Empty) => {}
            Err(err) => tracing::debug!(%err, line, "ignored"),
        }
    }
    anyhow::bail!("serial port closed")
}
