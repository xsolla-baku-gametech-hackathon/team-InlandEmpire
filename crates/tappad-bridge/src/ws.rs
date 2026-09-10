//! WebSocket fan-out. Every connected game gets every line.

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::tungstenite::Message;

/// Accepts connections on `listener` forever, one task per client.
pub async fn serve(listener: TcpListener, tx: broadcast::Sender<String>) {
    loop {
        match listener.accept().await {
            Ok((stream, peer)) => {
                tracing::info!(%peer, "game connected");
                tokio::spawn(client(stream, tx.subscribe()));
            }
            Err(err) => tracing::warn!(%err, "accept failed"),
        }
    }
}

async fn client(stream: TcpStream, mut rx: broadcast::Receiver<String>) {
    let Ok(ws) = tokio_tungstenite::accept_async(stream).await else {
        tracing::warn!("handshake failed");
        return;
    };
    let (mut sink, mut incoming) = ws.split();
    loop {
        tokio::select! {
            line = rx.recv() => match line {
                Ok(line) => {
                    if sink.send(Message::text(line)).await.is_err() {
                        break;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => tracing::warn!(n, "client lagged"),
                Err(broadcast::error::RecvError::Closed) => break,
            },
            msg = incoming.next() => match msg {
                None | Some(Ok(Message::Close(_)) | Err(_)) => break,
                Some(Ok(_)) => {}
            },
        }
    }
    tracing::info!("game disconnected");
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tappad_protocol::{parse_line, PadEvent};

    use super::*;
    use crate::source::run_fake;

    #[tokio::test]
    async fn game_receives_ready_then_tap() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let (tx, _) = broadcast::channel(16);
        tokio::spawn(serve(listener, tx.clone()));
        let (ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}")).await?;
        let (_, mut rx) = ws.split();
        tokio::spawn(run_fake(tx, Duration::from_millis(10)));

        let first = rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("no frame"))??;
        let second = rx
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("no frame"))??;
        assert!(matches!(
            parse_line(first.to_text()?)?,
            PadEvent::Ready { .. }
        ));
        match parse_line(second.to_text()?)? {
            PadEvent::Tap { uid } => assert_eq!(uid.as_str(), "04A3B2C1"),
            other => anyhow::bail!("expected tap, got {other:?}"),
        }
        Ok(())
    }
}
