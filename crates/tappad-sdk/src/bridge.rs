//! Taps from `tappad-bridge`. Reconnects on its own: the bridge may start after
//! the game, or the pad may be unplugged and plugged back in.

use std::time::Duration;

use futures_util::StreamExt;
use tappad_protocol::{parse_line, CardUid, PadEvent};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

use crate::Config;

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// The pad, as seen through the bridge. Holds one WebSocket and reopens it
/// whenever it drops, so [`Pad::next_event`] never fails; it only waits.
#[derive(Debug)]
pub struct Pad {
    url: String,
    retry: Duration,
    socket: Option<Socket>,
}

impl Pad {
    /// Points at the bridge named in `config`. Nothing is opened until the
    /// first call to [`Pad::next_event`].
    #[must_use]
    pub fn new(config: &Config) -> Self {
        Pad {
            url: config.bridge_url.clone(),
            retry: config.bridge_retry,
            socket: None,
        }
    }

    /// The next event from the pad: `ready`, `tap` or `error`.
    ///
    /// Frames that are not a pad event are dropped with a log line. When the
    /// bridge is not there, this waits [`Config::bridge_retry`] and tries again,
    /// for as long as the caller keeps waiting.
    pub async fn next_event(&mut self) -> PadEvent {
        loop {
            let Some(socket) = self.socket.as_mut() else {
                self.socket = Some(self.connect().await);
                continue;
            };
            match socket.next().await {
                Some(Ok(Message::Text(text))) => match parse_line(&text) {
                    Ok(event) => return event,
                    Err(err) => tracing::debug!(%err, "dropped a frame that is not a pad event"),
                },
                Some(Ok(Message::Close(_))) | None => {
                    tracing::warn!("bridge closed the connection");
                    self.socket = None;
                }
                Some(Ok(_)) => {}
                Some(Err(err)) => {
                    tracing::warn!(%err, "bridge connection failed");
                    self.socket = None;
                }
            }
        }
    }

    /// The next card held on the pad. `ready` and `error` events are logged
    /// and skipped.
    pub async fn next_tap(&mut self) -> CardUid {
        loop {
            match self.next_event().await {
                PadEvent::Tap { uid } => return uid,
                PadEvent::Ready { firmware } => tracing::info!(%firmware, "pad ready"),
                PadEvent::Error { message } => tracing::warn!(%message, "pad error"),
            }
        }
    }

    /// Keeps dialling until the bridge answers.
    async fn connect(&self) -> Socket {
        loop {
            match tokio_tungstenite::connect_async(&self.url).await {
                Ok((socket, _)) => {
                    tracing::info!(url = %self.url, "connected to the bridge");
                    return socket;
                }
                Err(err) => {
                    tracing::warn!(%err, url = %self.url, "bridge not reachable, retrying");
                    tokio::time::sleep(self.retry).await;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;
    use tokio::sync::broadcast;

    use super::*;

    /// A fake bridge tapping every 10 ms on a free port.
    fn fake_bridge(listener: TcpListener) {
        let (tx, _) = broadcast::channel(16);
        tokio::spawn(tappad_bridge::ws::serve(listener, tx.clone()));
        tokio::spawn(tappad_bridge::source::run_fake(
            tx,
            Duration::from_millis(10),
        ));
    }

    fn config_for(addr: std::net::SocketAddr) -> Config {
        Config {
            bridge_url: format!("ws://{addr}"),
            bridge_retry: Duration::from_millis(20),
            ..Config::default()
        }
    }

    #[tokio::test]
    async fn a_fake_tap_arrives_as_a_card_uid() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        fake_bridge(listener);
        let mut pad = Pad::new(&config_for(addr));
        let uid = tokio::time::timeout(Duration::from_secs(5), pad.next_tap()).await?;
        assert_eq!(uid.as_str().len() % 2, 0, "{uid} is not whole bytes");
        Ok(())
    }

    #[tokio::test]
    async fn a_bridge_that_starts_late_is_found() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        drop(listener);
        let mut pad = Pad::new(&config_for(addr));
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            if let Ok(listener) = TcpListener::bind(addr).await {
                fake_bridge(listener);
            }
        });
        tokio::time::timeout(Duration::from_secs(5), pad.next_tap()).await?;
        Ok(())
    }
}
