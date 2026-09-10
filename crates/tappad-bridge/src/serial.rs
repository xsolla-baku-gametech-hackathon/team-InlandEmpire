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
/// Bytes that are not UTF-8 are replaced, not fatal: an ESP32 emits a few
/// garbage bytes on reset and the boot ROM prints in its own encoding. One bad
/// byte must not take the pad down for the rest of the demo.
///
/// # Errors
/// The port cannot be opened, or reading it fails.
pub fn run_serial(port: &str, tx: &broadcast::Sender<String>) -> anyhow::Result<()> {
    let reader = serialport::new(port, BAUD)
        .timeout(Duration::from_secs(3600))
        .open()?;
    tracing::info!(port, "serial open");
    let mut reader = BufReader::new(reader);
    let mut raw = Vec::new();
    loop {
        raw.clear();
        if reader.read_until(b'\n', &mut raw)? == 0 {
            anyhow::bail!("serial port closed");
        }
        let line = decode_line(&raw);
        match parse_line(&line) {
            Ok(event) => {
                tracing::info!(?event, "pad");
                let _ = tx.send(to_line(&event)?);
            }
            Err(LineError::Empty) => {}
            Err(err) => tracing::debug!(%err, line, "ignored"),
        }
    }
}

/// One serial line as text, with invalid bytes replaced so parsing can decide.
fn decode_line(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw).into_owned()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use tappad_protocol::PadEvent;

    #[test]
    fn garbage_bytes_do_not_break_the_line_after_them() {
        // Reset noise, then a real tap line. The noise becomes U+FFFD and is rejected as
        // not a pad event; the tap still parses.
        let noise = decode_line(b"\xff\xfe\x00garbage\r\n");
        assert!(matches!(parse_line(&noise), Err(LineError::Json(_))));
        let tap = decode_line(b"{\"event\":\"tap\",\"uid\":\"C95DD006\"}\r\n");
        assert!(matches!(parse_line(&tap), Ok(PadEvent::Tap { .. })));
    }

    #[test]
    fn a_bad_byte_inside_a_line_only_loses_that_line() {
        let line = decode_line(b"{\"event\":\"tap\",\"uid\":\"C9\xff5DD006\"}\n");
        assert!(parse_line(&line).is_err());
    }
}
