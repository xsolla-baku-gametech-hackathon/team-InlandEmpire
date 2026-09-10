//! The one error type a game sees from this crate.

use tappad_protocol::OrderId;

/// Why a call into the SDK failed. A declined purchase is not an error; it is
/// a [`PurchaseResponse::Declined`](tappad_protocol::PurchaseResponse::Declined).
#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    /// The server could not be reached or the connection broke.
    #[error("server unreachable: {0}")]
    Transport(String),
    /// The server answered with a non-2xx status and an `{"error":...}` body.
    #[error("server answered {status}: {message}")]
    Server {
        /// HTTP status code.
        status: u16,
        /// The `error` field of the body, or the status text when there was none.
        message: String,
    },
    /// The server answered 2xx with a body that is not the documented shape.
    #[error("server answer did not match the protocol: {0}")]
    Protocol(String),
    /// The order did not reach a final state before the poll deadline.
    #[error("order {0} still not final after the poll timeout")]
    PollTimeout(OrderId),
}
