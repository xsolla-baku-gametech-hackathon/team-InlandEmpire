//! Message shapes from `docs/protocol.md`, mirrored here until `tappad-protocol` lands.
//!
//! Every type serialises exactly as the doc shows. When C's crate is on `dev`
//! this module becomes `pub use tappad_protocol::*;` and nothing else changes.

use std::fmt;

use serde::{Deserialize, Serialize};

/// NFC card UID, uppercase hex without separators.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardUid(String);

impl CardUid {
    /// Normalises `04:a3 b2-c1` style input to `04A3B2C1`.
    pub fn parse(raw: &str) -> Option<Self> {
        let hex: String = raw
            .chars()
            .filter(char::is_ascii_hexdigit)
            .map(|c| c.to_ascii_uppercase())
            .collect();
        (!hex.is_empty()).then_some(Self(hex))
    }

    /// The normalised hex string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Catalogue item id, for example `gems_500`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Sku(pub String);

/// Money in integer cents. Never a float.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Cents(pub u64);

impl fmt::Display for Cents {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "${}.{:02}", self.0 / 100, self.0 % 100)
    }
}

/// Provider-side order id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OrderId(pub u64);

/// Body of `POST /purchase`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurchaseRequest {
    /// Card that was tapped.
    pub uid: String,
    /// Item the player clicked.
    pub sku: Sku,
}

/// Why a purchase was refused before any money moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclineReason {
    /// Card is not in the registry.
    UnknownCard,
    /// Item price is above the card's per-tap limit.
    LimitExceeded,
    /// Provider refused the charge.
    InsufficientFunds,
    /// Item is not in the catalogue.
    UnknownSku,
}

/// Answer to `POST /purchase`. A decline is a normal answer, not an error.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PurchaseResponse {
    /// Provider created an order; the player must confirm at `checkout_url`.
    PendingPayment {
        /// Provider order id, poll it on `GET /orders/{id}`.
        order_id: OrderId,
        /// Pay Station URL to load in the game's iframe.
        checkout_url: String,
    },
    /// Paid without a checkout step.
    Approved {
        /// Provider order id.
        order_id: OrderId,
        /// Receipt reference to show the player.
        receipt_id: String,
    },
    /// Refused before any money moved.
    Declined {
        /// What stopped it.
        reason: DeclineReason,
    },
}

/// Lifecycle of a provider order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrderState {
    /// Created, not paid.
    New,
    /// Paid, fulfilment pending.
    Paid,
    /// Paid and fulfilled.
    Done,
    /// Player or provider cancelled it.
    Canceled,
    /// Token or order timed out.
    Expired,
}

impl OrderState {
    /// True once the player has paid.
    #[must_use]
    pub fn is_settled(self) -> bool {
        matches!(self, Self::Paid | Self::Done)
    }
}

/// Answer to `GET /orders/{id}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderStatus {
    /// Provider order id.
    pub order_id: OrderId,
    /// Where the order is now.
    pub state: OrderState,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uid_normalises_separators_and_case() {
        let uid = CardUid::parse("04:a3 b2-c1").map(|u| u.as_str().to_owned());
        assert_eq!(uid.as_deref(), Some("04A3B2C1"));
        assert_eq!(CardUid::parse("--"), None);
    }

    #[test]
    fn cents_display() {
        assert_eq!(Cents(499).to_string(), "$4.99");
        assert_eq!(Cents(5).to_string(), "$0.05");
    }

    #[test]
    fn responses_match_protocol_doc() -> serde_json::Result<()> {
        let declined = PurchaseResponse::Declined {
            reason: DeclineReason::LimitExceeded,
        };
        assert_eq!(
            serde_json::to_string(&declined)?,
            r#"{"status":"declined","reason":"limit_exceeded"}"#
        );
        let pending: PurchaseResponse = serde_json::from_str(
            r#"{"status":"pending_payment","order_id":12345,"checkout_url":"https://x"}"#,
        )?;
        assert_eq!(
            pending,
            PurchaseResponse::PendingPayment {
                order_id: OrderId(12345),
                checkout_url: "https://x".into(),
            }
        );
        Ok(())
    }
}
