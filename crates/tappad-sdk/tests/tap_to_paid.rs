//! The whole loop through the SDK alone: a fake bridge taps, a real server with
//! the mock provider answers, and the order is final.

use std::sync::Arc;
use std::time::Duration;

use tappad_sdk::{Config, PadEvent, PurchaseResponse, Sku, TapPad};
use tappad_server::provider::MockProvider;
use tappad_server::registry::Registry;
use tappad_server::routes::{router, AppState};
use tokio::net::TcpListener;
use tokio::sync::broadcast;

#[tokio::test]
async fn a_tap_buys_gems_and_the_order_settles() -> anyhow::Result<()> {
    let bridge = TcpListener::bind("127.0.0.1:0").await?;
    let bridge_addr = bridge.local_addr()?;
    let (tx, _) = broadcast::channel(16);
    tokio::spawn(tappad_bridge::ws::serve(bridge, tx.clone()));
    tokio::spawn(tappad_bridge::source::run_fake(
        tx,
        Duration::from_millis(10),
    ));

    let server = TcpListener::bind("127.0.0.1:0").await?;
    let server_addr = server.local_addr()?;
    let state = AppState {
        registry: Arc::new(Registry::demo()?),
        provider: Arc::new(MockProvider::default()),
    };
    tokio::spawn(async move { axum::serve(server, router(state)).await });

    let mut tappad = TapPad::connect(&Config {
        server_url: format!("http://{server_addr}"),
        bridge_url: format!("ws://{bridge_addr}"),
        ..Config::default()
    })?;

    let items = tappad.catalog().await?;
    let sku = Sku::new("gems_500");
    assert!(items.iter().any(|i| i.sku == sku), "shop must sell {sku}");

    let uid = tokio::time::timeout(Duration::from_secs(5), tappad.next_tap()).await?;
    let order_id = match tappad.buy(uid, sku).await? {
        PurchaseResponse::Approved { order_id, .. }
        | PurchaseResponse::PendingPayment { order_id, .. } => order_id,
        PurchaseResponse::Declined { reason } => anyhow::bail!("declined: {reason:?}"),
    };
    assert!(tappad.wait_for_payment(order_id).await?.is_success());
    Ok(())
}

#[tokio::test]
async fn the_fake_pad_says_ready_before_its_first_tap() -> anyhow::Result<()> {
    let bridge = TcpListener::bind("127.0.0.1:0").await?;
    let bridge_addr = bridge.local_addr()?;
    let (tx, _) = broadcast::channel(16);
    tokio::spawn(tappad_bridge::ws::serve(bridge, tx.clone()));

    let mut tappad = TapPad::connect(&Config {
        bridge_url: format!("ws://{bridge_addr}"),
        ..Config::default()
    })?;
    // Connect first, then start the pad, so the ready line is not missed.
    let first = tokio::spawn(async move {
        let event = tappad.next_event().await;
        (event, tappad)
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    tokio::spawn(tappad_bridge::source::run_fake(
        tx,
        Duration::from_millis(10),
    ));

    let (event, mut tappad) = tokio::time::timeout(Duration::from_secs(5), first).await??;
    assert!(matches!(event, PadEvent::Ready { .. }), "{event:?}");
    let next = tokio::time::timeout(Duration::from_secs(5), tappad.next_event()).await?;
    assert!(matches!(next, PadEvent::Tap { .. }), "{next:?}");
    Ok(())
}
