//! The real pad: one JSON line per event over USB serial.

use std::io::{BufRead, BufReader, ErrorKind, Read};
use std::time::Duration;

use tappad_protocol::{parse_line, LineError};
use tokio::sync::broadcast;

use crate::source::to_line;

/// Baud rate the firmware uses.
pub const BAUD: u32 = 115_200;

/// How long one read waits before looping. Short, because a read that returns
/// `TimedOut` is how we notice the process should keep going: a pad that is not
/// tapped is silent for minutes, and that is not an error.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// How long to wait before opening the port again after it went away.
const REOPEN_DELAY: Duration = Duration::from_secs(1);

/// Opens `port` and forwards every well-formed line, reopening the port if the
/// cable is pulled. Blocking; run it on its own thread.
///
/// Bytes that are not UTF-8 are replaced, not fatal: an ESP32 emits a few
/// garbage bytes on reset and the boot ROM prints in its own encoding. One bad
/// byte must not take the pad down for the rest of the demo.
///
/// # Errors
/// The port cannot be opened the first time, which is a wrong `--port` and worth
/// stopping for. Later failures are logged and retried instead.
pub fn run_serial(port: &str, tx: &broadcast::Sender<String>) -> anyhow::Result<()> {
    let mut reader = open(port)?;
    loop {
        if let Err(err) = pump(&mut reader, tx) {
            tracing::warn!(port, %err, "serial port lost, reopening every {REOPEN_DELAY:?}");
        }
        reader = loop {
            std::thread::sleep(REOPEN_DELAY);
            match open(port) {
                Ok(reader) => break reader,
                Err(err) => tracing::debug!(port, %err, "reopen failed"),
            }
        };
    }
}

/// Opens the port with the read timeout this module expects.
fn open(port: &str) -> anyhow::Result<BufReader<Box<dyn serialport::SerialPort>>> {
    let port_handle = serialport::new(port, BAUD).timeout(READ_TIMEOUT).open()?;
    tracing::info!(port, "serial open");
    Ok(BufReader::new(port_handle))
}

/// Reads lines until the port goes away. A read timeout means the pad is simply
/// idle, so it keeps waiting and keeps whatever half a line it already has.
fn pump<R: Read>(reader: &mut BufReader<R>, tx: &broadcast::Sender<String>) -> anyhow::Result<()> {
    let mut raw = Vec::new();
    loop {
        match reader.read_until(b'\n', &mut raw) {
            Ok(0) => anyhow::bail!("serial port closed"),
            Ok(_) => {}
            Err(e) if e.kind() == ErrorKind::TimedOut => continue,
            Err(e) => return Err(e.into()),
        }
        if !raw.ends_with(b"\n") {
            // A timeout can cut a line in half; wait for the rest.
            continue;
        }
        let line = decode_line(&raw);
        raw.clear();
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

    /// Hands out canned reads, so a test can put a timeout in the middle of a line.
    struct Script(Vec<std::io::Result<&'static [u8]>>);

    impl Read for Script {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            match self.0.remove(0) {
                Ok(bytes) => {
                    buf[..bytes.len()].copy_from_slice(bytes);
                    Ok(bytes.len())
                }
                Err(e) => Err(e),
            }
        }
    }

    fn timed_out() -> std::io::Error {
        std::io::Error::new(ErrorKind::TimedOut, "no data")
    }

    #[test]
    fn a_timeout_in_the_middle_of_a_line_keeps_the_half_it_has() {
        let (tx, mut rx) = broadcast::channel(4);
        let script = Script(vec![
            Ok(b"{\"event\":\"tap\","),
            Err(timed_out()),
            Err(timed_out()),
            Ok(b"\"uid\":\"C95DD006\"}\n"),
            Ok(b""),
        ]);
        let err = pump(&mut BufReader::new(script), &tx).unwrap_err();
        assert!(err.to_string().contains("closed"), "{err}");
        let line = rx.try_recv().unwrap();
        assert!(
            matches!(parse_line(&line), Ok(PadEvent::Tap { .. })),
            "{line}"
        );
    }

    #[test]
    fn an_idle_pad_is_not_an_error() {
        let (tx, _rx) = broadcast::channel(4);
        let script = Script(vec![Err(timed_out()), Err(timed_out()), Ok(b"")]);
        let err = pump(&mut BufReader::new(script), &tx).unwrap_err();
        assert!(err.to_string().contains("closed"), "{err}");
    }
}
