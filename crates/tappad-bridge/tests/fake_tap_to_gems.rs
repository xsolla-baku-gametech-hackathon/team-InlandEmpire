//! Plays the game: takes a tap from the fake bridge, posts it to a real server, expects gems.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tappad_bridge::{source, ws};
use tappad_protocol::{parse_line, PadEvent, PurchaseRequest, PurchaseResponse, Sku};
use tappad_server::provider::MockProvider;
use tappad_server::registry::Registry;
use tappad_server::routes::{router, AppState};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

#[tokio::test]
async fn fake_tap_gives_gems_with_mock_provider() -> anyhow::Result<()> {
    let bridge = TcpListener::bind("127.0.0.1:0").await?;
    let bridge_addr = bridge.local_addr()?;
    let (tx, _) = broadcast::channel(16);
    tokio::spawn(ws::serve(bridge, tx.clone()));
    tokio::spawn(source::run_fake(tx, Duration::from_millis(10)));

    let registry = Arc::new(Registry::demo()?);
    let server = TcpListener::bind("127.0.0.1:0").await?;
    let server_addr = server.local_addr()?;
    let state = AppState {
        registry: registry.clone(),
        provider: Arc::new(MockProvider::default()),
    };
    tokio::spawn(async move { axum::serve(server, router(state)).await });

    let (socket, _) = tokio_tungstenite::connect_async(format!("ws://{bridge_addr}")).await?;
    let (_, mut frames) = socket.split();
    let uid = loop {
        let frame = frames
            .next()
            .await
            .ok_or_else(|| anyhow::anyhow!("bridge closed"))??;
        if let PadEvent::Tap { uid } = parse_line(frame.to_text()?)? {
            break uid;
        }
    };

    let sku = Sku::new("gems_500");
    let answer: PurchaseResponse = reqwest::Client::new()
        .post(format!("http://{server_addr}/purchase"))
        .json(&PurchaseRequest {
            uid,
            sku: sku.clone(),
        })
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let PurchaseResponse::Approved { .. } = answer else {
        anyhow::bail!("expected approved, got {answer:?}");
    };
    assert_eq!(registry.gems_for(&sku), Some(500));
    Ok(())
}
