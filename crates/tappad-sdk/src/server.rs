//! HTTP client for `tappad-server`. Three routes, shapes in `docs/protocol.md`.

use serde::de::DeserializeOwned;
use tappad_protocol::{CatalogItem, ErrorBody};

use crate::{Config, SdkError};

/// Talks to `tappad-server`. Cheap to clone; one per game is enough.
#[derive(Debug, Clone)]
pub struct ServerClient {
    http: reqwest::Client,
    base: String,
}

impl ServerClient {
    /// Builds a client for the server named in `config`.
    ///
    /// # Errors
    /// The HTTP client could not be built, for example when TLS fails to initialise.
    pub fn new(config: &Config) -> Result<Self, SdkError> {
        let http = reqwest::Client::builder()
            .build()
            .map_err(|e| SdkError::Transport(e.to_string()))?;
        Ok(ServerClient {
            http,
            base: config.server_url.trim_end_matches('/').to_owned(),
        })
    }

    /// `GET /catalog`: what the shop can sell, in store order.
    ///
    /// # Errors
    /// [`SdkError::Transport`] when the server is down, [`SdkError::Server`] on a
    /// non-2xx answer, [`SdkError::Protocol`] on a body that is not a catalogue.
    pub async fn catalog(&self) -> Result<Vec<CatalogItem>, SdkError> {
        let res = self.http.get(format!("{}/catalog", self.base)).send().await;
        read(res).await
    }
}

/// Turns a reqwest answer into the typed body, or the matching [`SdkError`].
async fn read<T: DeserializeOwned>(
    res: Result<reqwest::Response, reqwest::Error>,
) -> Result<T, SdkError> {
    let res = res.map_err(|e| SdkError::Transport(e.to_string()))?;
    let status = res.status();
    if !status.is_success() {
        let message = match res.json::<ErrorBody>().await {
            Ok(body) => body.error,
            Err(_) => status
                .canonical_reason()
                .unwrap_or("no error body")
                .to_owned(),
        };
        return Err(SdkError::Server {
            status: status.as_u16(),
            message,
        });
    }
    res.json::<T>()
        .await
        .map_err(|e| SdkError::Protocol(e.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use tappad_server::provider::MockProvider;
    use tappad_server::registry::Registry;
    use tappad_server::routes::{router, AppState};
    use tokio::net::TcpListener;

    use super::*;

    /// A real server on a free loopback port, mock provider, demo cards.
    pub(crate) async fn demo_server() -> anyhow::Result<Config> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let state = AppState {
            registry: Arc::new(Registry::demo()?),
            provider: Arc::new(MockProvider::default()),
        };
        tokio::spawn(async move { axum::serve(listener, router(state)).await });
        Ok(Config {
            server_url: format!("http://{addr}"),
            ..Config::default()
        })
    }

    #[tokio::test]
    async fn catalog_lists_the_gem_packs() -> anyhow::Result<()> {
        let client = ServerClient::new(&demo_server().await?)?;
        let items = client.catalog().await?;
        assert!(items.iter().any(|i| i.sku.as_str() == "gems_500"));
        Ok(())
    }

    #[tokio::test]
    async fn a_dead_server_is_a_transport_error() -> anyhow::Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        drop(listener);
        let client = ServerClient::new(&Config {
            server_url: format!("http://{addr}"),
            ..Config::default()
        })?;
        assert!(matches!(
            client.catalog().await,
            Err(SdkError::Transport(_))
        ));
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_route_is_a_server_error_with_its_message() -> anyhow::Result<()> {
        let config = demo_server().await?;
        let client = ServerClient::new(&config)?;
        let res = client
            .http
            .get(format!("{}/nope", client.base))
            .send()
            .await;
        let err = read::<Vec<CatalogItem>>(res).await.err();
        match err {
            Some(SdkError::Server { status, message }) => {
                assert_eq!(status, 404);
                assert_eq!(message, "no such route");
            }
            other => anyhow::bail!("expected a server error, got {other:?}"),
        }
        Ok(())
    }
}
