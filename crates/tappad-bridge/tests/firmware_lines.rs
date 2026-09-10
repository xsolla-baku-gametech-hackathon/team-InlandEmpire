//! Pins the exact lines `firmware/tappad_pad/tappad_pad.ino` prints, byte for byte,
//! against the parser the bridge feeds them to. Change the sketch and this file together.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use tappad_protocol::{parse_line, LineError, PadEvent};

#[test]
fn boot_line_is_ready() {
    let event = parse_line(r#"{"event":"ready","firmware":"0.1.0"}"#).unwrap();
    assert_eq!(
        event,
        PadEvent::Ready {
            firmware: "0.1.0".into()
        }
    );
}

#[test]
fn tap_line_carries_the_uid_as_printed() {
    // Four-byte UID, the common case for MIFARE Classic cards.
    let event = parse_line(r#"{"event":"tap","uid":"04A3B2C1"}"#).unwrap();
    let PadEvent::Tap { uid } = event else {
        panic!("expected tap")
    };
    assert_eq!(uid.to_string(), "04A3B2C1");

    // Seven-byte UID, MIFARE Ultralight and NTAG stickers.
    let event = parse_line(r#"{"event":"tap","uid":"04A3B2C1D2E3F4"}"#).unwrap();
    let PadEvent::Tap { uid } = event else {
        panic!("expected tap")
    };
    assert_eq!(uid.to_string(), "04A3B2C1D2E3F4");
}

#[test]
fn reader_missing_is_an_error_event() {
    let event = parse_line(r#"{"event":"error","message":"reader not found"}"#).unwrap();
    assert_eq!(
        event,
        PadEvent::Error {
            message: "reader not found".into()
        }
    );
}

#[test]
fn serial_line_endings_do_not_matter() {
    // Arduino's println sends CRLF; the bridge's BufRead::lines strips LF, so a CR survives.
    let event = parse_line("{\"event\":\"tap\",\"uid\":\"04A3B2C1\"}\r").unwrap();
    assert!(matches!(event, PadEvent::Tap { .. }));
}

#[test]
fn lines_from_the_old_sketch_are_rejected_not_forwarded() {
    // The pre-protocol sketch printed these. None may reach the game as an event.
    for line in ["READY", "PROCESSING", "PAYMENT:12.34"] {
        assert!(
            !matches!(parse_line(line), Ok(_) | Err(LineError::Empty)),
            "{line} must be a parse error"
        );
    }
    assert!(matches!(parse_line(""), Err(LineError::Empty)));
    assert!(matches!(parse_line("\r"), Err(LineError::Empty)));
}
