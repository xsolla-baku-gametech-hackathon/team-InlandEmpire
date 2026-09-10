//! Starts the HTTP server. Configuration comes from `.env` or the environment.

use std::sync::Arc;

use anyhow::Context;
use tappad_server::provider::{MockProvider, PaymentProvider};
use tappad_server::registry::Registry;
use tappad_server::routes::{router, AppState};
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
        "xsolla" => Arc::new(XsollaProvider::new(XsollaConfig::from_env().context(
            "TAPPAD_PROVIDER=xsolla needs XSOLLA_PROJECT_ID and XSOLLA_API_KEY",
        )?)),
        other => anyhow::bail!("TAPPAD_PROVIDER={other} is not one of: mock, xsolla"),
    };
    let addr = std::env::var("TAPPAD_SERVER_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());

    let state = AppState {
        registry: Arc::new(Registry::demo()),
        provider,
    };
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;
    tracing::info!(%addr, provider = %provider_name, "tappad-server listening");
    axum::serve(listener, router(state)).await?;
    Ok(())
}
