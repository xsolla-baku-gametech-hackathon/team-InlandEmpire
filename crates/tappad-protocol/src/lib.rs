//! Shared message types. The JSON shapes here are the contract between
//! firmware, bridge, server and game; see `docs/protocol.md`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Where the bridge serves WebSocket frames and the game connects.
pub const BRIDGE_WS_ADDR: &str = "127.0.0.1:8765";

/// Where the server listens for `POST /purchase` and `GET /orders/{id}`.
pub const SERVER_ADDR: &str = "127.0.0.1:8080";

// ------------------------------------------------------------------- money

/// An amount of money in whole cents. On the wire it is a bare integer: `499`.
///
/// Never a float: `4.99` cannot be stored exactly in binary, and serde
/// rejects floats and negatives for `u64` on its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cents(pub u64);

impl Cents {
    /// Adds two amounts, or returns `None` if the sum does not fit in `u64`.
    #[must_use]
    pub fn checked_add(self, other: Cents) -> Option<Cents> {
        self.0.checked_add(other.0).map(Cents)
    }
}

impl fmt::Display for Cents {
    /// Prints `499` as `4.99`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}

// ---------------------------------------------------------------- card uid

/// Why a string is not a card UID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UidError {
    /// A character that is not a hex digit was found after removing separators.
    #[error("card uid contains a character that is not a hex digit")]
    NotHex,
    /// The number of hex digits does not match a 4, 7 or 10 byte UID.
    #[error("card uid has {0} hex digits, expected 8, 14 or 20")]
    BadLength(usize),
}

/// The unique id of an NFC card, as uppercase hex with no separators: `04A1B2C3`.
///
/// Parsing accepts lowercase and `:`, `-` or space separators, so `04:a1:b2:c3`
/// is the same card. Only 4, 7 and 10 byte UIDs exist for the cards we use.
/// Validation also runs when a UID arrives inside JSON.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CardUid(String);

impl CardUid {
    /// The normalised UID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for CardUid {
    type Err = UidError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let digits: String = raw
            .chars()
            .filter(|c| !matches!(c, ':' | '-' | ' '))
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(UidError::NotHex);
        }
        match digits.len() {
            8 | 14 | 20 => Ok(CardUid(digits)),
            n => Err(UidError::BadLength(n)),
        }
    }
}

impl TryFrom<String> for CardUid {
    type Error = UidError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        raw.parse()
    }
}

impl From<CardUid> for String {
    fn from(uid: CardUid) -> Self {
        uid.0
    }
}

impl fmt::Display for CardUid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// --------------------------------------------------------------- other ids

/// The catalogue id of an item, such as `gems_500`. On the wire it is a string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Sku(String);

impl Sku {
    /// Wraps a catalogue id.
    pub fn new(sku: impl Into<String>) -> Self {
        Sku(sku.into())
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sku {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// The id Xsolla gives an order. On the wire it is a bare integer: `12345`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderId(pub u64);

impl fmt::Display for OrderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The receipt id returned with an approved purchase. On the wire it is a string.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReceiptId(String);

impl ReceiptId {
    /// Wraps a receipt id.
    pub fn new(id: impl Into<String>) -> Self {
        ReceiptId(id.into())
    }

    /// The id as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ReceiptId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// --------------------------------------------------------------- pad events

/// One message from the pad, tagged by `event`. The firmware prints one per
/// line over serial and the bridge forwards it unchanged over WebSocket.
///
/// Wire: `{"event":"ready","firmware":"0.1.0"}`,
/// `{"event":"tap","uid":"04A3B2C1"}`,
/// `{"event":"error","message":"reader timeout"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PadEvent {
    /// The firmware booted and the reader answers.
    Ready {
        /// Firmware version string, for the log.
        firmware: String,
    },
    /// A card was held on the pad.
    Tap {
        /// The card that was tapped.
        uid: CardUid,
    },
    /// The reader failed; the pad keeps running.
    Error {
        /// Human readable cause, for the log.
        message: String,
    },
}

/// Why a serial line is not a [`PadEvent`].
#[derive(Debug, thiserror::Error)]
pub enum LineError {
    /// The line was empty after trimming whitespace.
    #[error("empty line")]
    Empty,
    /// The line was not the JSON of a known event. Boot noise from the chip,
    /// unknown `event` values and bad UIDs all land here.
    #[error("not a pad event: {0}")]
    Json(#[from] serde_json::Error),
}

/// Parses one line read from the serial port into a [`PadEvent`].
///
/// Surrounding whitespace such as `\r\n` is ignored and extra JSON fields are
/// tolerated, so the firmware can add debug fields without breaking the bridge.
///
/// # Errors
///
/// Returns [`LineError::Empty`] for a blank line and [`LineError::Json`] for
/// anything that is not one of the known events, including chip boot noise.
pub fn parse_line(line: &str) -> Result<PadEvent, LineError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(LineError::Empty);
    }
    Ok(serde_json::from_str(line)?)
}

#[cfg(test)]
mod tests;
