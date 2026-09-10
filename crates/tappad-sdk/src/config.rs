//! Where the bridge and server live, and how patiently to poll.

use std::time::Duration;

use tappad_protocol::{BRIDGE_WS_ADDR, SERVER_ADDR};

/// Addresses and timings. [`Config::default`] matches a `make demo` laptop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Base URL of `tappad-server`, no trailing slash: `http://127.0.0.1:8080`.
    pub server_url: String,
    /// WebSocket URL of `tappad-bridge`: `ws://127.0.0.1:8765`.
    pub bridge_url: String,
    /// Gap between two `GET /orders/{id}` calls while waiting for a payment.
    pub poll_interval: Duration,
    /// How long to wait for an order to become final before giving up.
    pub poll_timeout: Duration,
    /// Gap before reconnecting to a bridge that went away.
    pub bridge_retry: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            server_url: format!("http://{SERVER_ADDR}"),
            bridge_url: format!("ws://{BRIDGE_WS_ADDR}"),
            poll_interval: Duration::from_millis(800),
            poll_timeout: Duration::from_secs(5 * 60),
            bridge_retry: Duration::from_secs(2),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_point_at_the_demo_ports() {
        let c = Config::default();
        assert_eq!(c.server_url, "http://127.0.0.1:8080");
        assert_eq!(c.bridge_url, "ws://127.0.0.1:8765");
        assert!(c.poll_interval < c.poll_timeout);
    }
}
