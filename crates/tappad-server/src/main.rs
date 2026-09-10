//! Starts the HTTP server. Configuration comes from `.env` or the environment.

use std::sync::Arc;

use anyhow::Context;
use tappad_server::provider::{MockProvider, PaymentProvider};
use tappad_server::registry::{Registry, DEFAULT_CARD_CAP};
use tappad_server::routes::{router, AppState};
use tappad_server::types::Cents;
use tappad_server::xsolla::{XsollaConfig, XsollaProvider};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,tappad_server=debug".into()),
        )
        .init();

    let provider_name = std::env::var("TAPPAD_PROVIDER").unwrap_or_else(|_| "mock".into());
    let provider: Arc<dyn PaymentProvider> = match provider_name.as_str() {
        "mock" => Arc::new(MockProvider::default()),
        "xsolla" => {
            let config = XsollaConfig::from_env()
                .context("TAPPAD_PROVIDER=xsolla needs XSOLLA_PROJECT_ID and XSOLLA_API_KEY")?;
            Arc::new(XsollaProvider::new(config)?)
        }
        other => anyhow::bail!("TAPPAD_PROVIDER={other} is not one of: mock, xsolla"),
    };
    let addr =
        std::env::var("TAPPAD_SERVER_ADDR").unwrap_or_else(|_| tappad_protocol::SERVER_ADDR.into());

    let cap = match std::env::var("TAPPAD_CARD_CAP_CENTS") {
        Ok(raw) => Cents(
            raw.parse()
                .with_context(|| format!("TAPPAD_CARD_CAP_CENTS={raw} is not a whole number"))?,
        ),
        Err(_) => DEFAULT_CARD_CAP,
    };
    let registry = Registry::demo()
        .context("a demo card UID does not parse")?
        .with_cap(cap);
    tracing::info!(%cap, "per-card spending cap for this run");
    let state = AppState {
        registry: Arc::new(registry),
        provider,
    };
    warn_on_price_drift(&state).await;

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;
    tracing::info!(%addr, provider = %provider_name, "tappad-server listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}

/// Compares the local catalogue against the provider's, because the token request
/// sends only a SKU: the provider charges its own price, and the card limit was
/// checked against ours. A difference is a warning, never a reason not to start.
async fn warn_on_price_drift(state: &AppState) {
    let items = match state.provider.catalog().await {
        Ok(items) => items,
        Err(err) => {
            tracing::warn!(%err, "cannot read the provider catalogue, prices unchecked");
            return;
        }
    };
    for item in &items {
        match state.registry.price_for(&item.sku) {
            Some(local) if local == item.price => {}
            Some(local) => tracing::warn!(
                sku = %item.sku,
                %local,
                store = %item.price,
                "price differs from the store; the card limit is checked against the local one"
            ),
            None => tracing::warn!(sku = %item.sku, "the store sells an item we do not"),
        }
    }
    for (sku, price) in state.registry.items() {
        if !items.iter().any(|i| &i.sku == sku) {
            tracing::warn!(%sku, %price, "we sell an item the store does not");
        }
    }
}
