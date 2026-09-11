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

impl SdkError {
    /// True when the same call may succeed if tried again: the server was
    /// unreachable, it failed on its side (5xx), or an order was still open at
    /// the deadline. A 4xx or a body that broke the protocol will not fix itself.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            SdkError::Transport(_) | SdkError::PollTimeout(_) => true,
            SdkError::Server { status, .. } => *status >= 500,
            SdkError::Protocol(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(status: u16) -> SdkError {
        SdkError::Server {
            status,
            message: String::new(),
        }
    }

    #[test]
    fn upstream_and_transport_failures_are_worth_a_retry() {
        assert!(SdkError::Transport("down".into()).is_retryable());
        assert!(server(502).is_retryable());
        assert!(SdkError::PollTimeout(OrderId(1)).is_retryable());
    }

    #[test]
    fn client_mistakes_and_protocol_breaks_are_not() {
        assert!(!server(404).is_retryable());
        assert!(!server(400).is_retryable());
        assert!(!SdkError::Protocol("bad body".into()).is_retryable());
    }
}
