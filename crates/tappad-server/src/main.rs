//! Starts the HTTP server. Configuration comes from `.env` or the environment.

use std::net::{SocketAddr, ToSocketAddrs};
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

    let allow_remote = std::env::var("TAPPAD_ALLOW_REMOTE").is_ok_and(|v| v == "1");
    let addr = bind_addr(&addr, allow_remote)?;
    if !addr.ip().is_loopback() {
        tracing::warn!(%addr, "TAPPAD_ALLOW_REMOTE=1: this server has no authentication and anyone who can reach it can spend a known card");
    }
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;
    tracing::info!(%addr, provider = %provider_name, "tappad-server listening");
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
            tracing::info!("stopping");
        })
        .await?;
    Ok(())
}

/// Resolves the address to listen on, refusing anything but loopback unless the
/// operator asked for it. The server has no authentication and the card UID is
/// not a secret, so binding it to a LAN interface hands out a spending endpoint.
///
/// # Errors
/// The address does not resolve, or it is not loopback and `allow_remote` is false.
fn bind_addr(raw: &str, allow_remote: bool) -> anyhow::Result<SocketAddr> {
    let addr = raw
        .to_socket_addrs()
        .with_context(|| format!("cannot resolve {raw}"))?
        .next()
        .with_context(|| format!("{raw} resolved to nothing"))?;
    anyhow::ensure!(
        allow_remote || addr.ip().is_loopback(),
        "refusing to listen on {addr}: it is not loopback.          Set TAPPAD_ALLOW_REMOTE=1 if a trusted LAN really should reach this server."
    );
    Ok(addr)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_is_always_allowed() -> anyhow::Result<()> {
        let addr = bind_addr("127.0.0.1:8080", false)?;
        assert!(addr.ip().is_loopback());
        assert_eq!(addr.port(), 8080);
        Ok(())
    }

    #[test]
    fn a_public_address_needs_the_flag() {
        let refused = bind_addr("0.0.0.0:8080", false);
        assert!(refused.is_err(), "0.0.0.0 must not bind by default");
        assert!(
            bind_addr("0.0.0.0:8080", true).is_ok(),
            "the flag allows it"
        );
    }

    #[test]
    fn nonsense_is_an_error_not_a_default() {
        assert!(bind_addr("not an address", false).is_err());
    }
}
