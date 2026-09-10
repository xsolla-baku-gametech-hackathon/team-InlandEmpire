//! Plays the game: takes a tap from the fake bridge, posts it to a real server,
//! and follows the order to a final state.
//!
//! Granting the gems happens in the game's JavaScript (`ui/shop.js`), which this
//! test cannot reach; the node tests cover that half. What is proved here is the
//! Rust half of the loop: a tap off the wire buys something and the order settles.

use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use tappad_bridge::{source, ws};
use tappad_protocol::{parse_line, OrderStatus, PadEvent, PurchaseRequest, PurchaseResponse, Sku};
use tappad_server::provider::MockProvider;
use tappad_server::registry::Registry;
use tappad_server::routes::{router, AppState};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

#[tokio::test]
async fn a_fake_tap_buys_and_the_order_reaches_a_final_state() -> anyhow::Result<()> {
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
    assert!(
        registry.gems_for(&sku).is_some(),
        "the demo registry must sell the item this test buys"
    );
    let http = reqwest::Client::new();
    let answer: PurchaseResponse = http
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
    let PurchaseResponse::Approved {
        order_id,
        receipt_id,
    } = answer
    else {
        anyhow::bail!("expected approved, got {answer:?}");
    };
    assert!(
        receipt_id.as_str().starts_with("rcpt-"),
        "receipt {receipt_id} does not look like one"
    );

    let status: OrderStatus = http
        .get(format!("http://{server_addr}/orders/{order_id}"))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    assert_eq!(status.order_id, order_id);
    assert!(
        status.state.is_final() && status.state.is_success(),
        "the game polls until a final state; got {:?}",
        status.state
    );
    Ok(())
}
