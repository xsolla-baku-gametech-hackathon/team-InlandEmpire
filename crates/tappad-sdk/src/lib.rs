//! Client library for a game that sells through `TapPad`.
//!
//! The game never speaks HTTP or WebSocket itself. It asks this crate for the
//! next tap, hands the tap and a SKU to the server, and follows the order to
//! a final state. The wire shapes come from `tappad-protocol` and are
//! re-exported here so a game depends on one crate.

mod bridge;
mod config;
mod error;
mod server;

pub use bridge::Pad;
pub use config::Config;
pub use error::SdkError;
pub use server::ServerClient;
pub use tappad_protocol::{
    CardUid, CatalogItem, Cents, DeclineReason, OrderId, OrderState, OrderStatus, PadEvent,
    PurchaseRequest, PurchaseResponse, ReceiptId, Sku, BRIDGE_WS_ADDR, SERVER_ADDR,
};
