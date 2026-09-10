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

// ----------------------------------------------------------------- purchase

/// Body of `POST /purchase`, sent by the game after a tap.
///
/// Wire: `{"uid":"04A3B2C1","sku":"gems_500"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurchaseRequest {
    /// The card that was tapped.
    pub uid: CardUid,
    /// The item the player clicked Buy on.
    pub sku: Sku,
}

/// Why the server declined a purchase before or instead of asking Xsolla.
///
/// Wire: `"unknown_card"`, `"limit_exceeded"`, `"insufficient_funds"`,
/// `"unknown_sku"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclineReason {
    /// The card is not in the registry.
    UnknownCard,
    /// The item costs more than this card may spend.
    LimitExceeded,
    /// The card's balance does not cover the item.
    InsufficientFunds,
    /// The item is not in the catalogue.
    UnknownSku,
}

impl DeclineReason {
    /// Every reason, so a test can prove each one has player-facing text.
    pub const ALL: [DeclineReason; 4] = [
        DeclineReason::UnknownCard,
        DeclineReason::LimitExceeded,
        DeclineReason::InsufficientFunds,
        DeclineReason::UnknownSku,
    ];

    /// The sentence the game shows the player. Lives here so it is typed once.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            DeclineReason::UnknownCard => "This card is not registered.",
            DeclineReason::LimitExceeded => "Over this card's spending limit.",
            DeclineReason::InsufficientFunds => "Not enough funds on this card.",
            DeclineReason::UnknownSku => "This item does not exist.",
        }
    }
}

/// Body of the `POST /purchase` answer, tagged by `status`. Always HTTP 200:
/// a decline is a business answer, not a server error.
///
/// Wire: `{"status":"pending_payment","order_id":12345,"checkout_url":"https://..."}`,
/// `{"status":"approved","order_id":7,"receipt_id":"rcpt-000007"}`,
/// `{"status":"declined","reason":"limit_exceeded"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PurchaseResponse {
    /// Xsolla created an order; the game opens `checkout_url` and polls.
    PendingPayment {
        /// The order to poll with `GET /orders/{id}`.
        order_id: OrderId,
        /// The Xsolla checkout page to load in the game's iframe.
        checkout_url: String,
    },
    /// Paid already, for example with the mock provider. Grant the item now.
    Approved {
        /// The order that was paid.
        order_id: OrderId,
        /// The receipt for the log.
        receipt_id: ReceiptId,
    },
    /// The registry said no. Show [`DeclineReason::message`] to the player.
    Declined {
        /// Why the purchase was declined.
        reason: DeclineReason,
    },
}

impl PurchaseResponse {
    /// Builds the declined answer for a reason.
    #[must_use]
    pub fn declined(reason: DeclineReason) -> Self {
        PurchaseResponse::Declined { reason }
    }
}

// ------------------------------------------------------------------- orders

/// Where an order is in its life. Wire: `"new"`, `"paid"`, `"done"`,
/// `"canceled"`, `"expired"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderState {
    /// Created, not paid yet. Keep polling.
    New,
    /// The player paid.
    Paid,
    /// Paid and fulfilled on Xsolla's side.
    Done,
    /// The player closed the checkout.
    Canceled,
    /// The checkout timed out.
    Expired,
}

impl OrderState {
    /// True when the state will not change again, so the game can stop polling.
    #[must_use]
    pub fn is_final(self) -> bool {
        !matches!(self, OrderState::New)
    }

    /// True when the player should get the item.
    #[must_use]
    pub fn is_success(self) -> bool {
        matches!(self, OrderState::Paid | OrderState::Done)
    }
}

/// Body of the `GET /orders/{id}` answer.
///
/// Wire: `{"order_id":12345,"state":"paid"}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderStatus {
    /// The order that was asked about.
    pub order_id: OrderId,
    /// Its current state.
    pub state: OrderState,
}

// -------------------------------------------------------------------- errors

/// Body of any non-2xx answer from the server, for example HTTP 502 when the
/// payment provider failed.
///
/// Wire: `{"error":"..."}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// What went wrong, in words safe to show in a log.
    pub error: String,
}

#[cfg(test)]
mod tests;
