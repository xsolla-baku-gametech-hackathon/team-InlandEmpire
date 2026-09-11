//! HTTP client for `tappad-server`. Three routes, shapes in `docs/protocol.md`.

use std::time::Duration;

use serde::de::DeserializeOwned;
use tappad_protocol::{
    CardUid, CatalogItem, ErrorBody, OrderId, OrderState, OrderStatus, PurchaseRequest,
    PurchaseResponse, Sku,
};

use crate::{Config, SdkError};

/// Talks to `tappad-server`. Cheap to clone; one per game is enough.
#[derive(Debug, Clone)]
pub struct ServerClient {
    http: reqwest::Client,
    base: String,
    poll_interval: Duration,
    poll_timeout: Duration,
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
            poll_interval: config.poll_interval,
            poll_timeout: config.poll_timeout,
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

    /// `POST /purchase`: buy `sku` with the card that was just tapped.
    ///
    /// A decline comes back as `Ok(PurchaseResponse::Declined { .. })`, not as an
    /// error: the server said no, it did not fail.
    ///
    /// # Errors
    /// [`SdkError::Server`] with status 502 when the payment provider failed;
    /// otherwise as for [`ServerClient::catalog`].
    pub async fn purchase(&self, uid: CardUid, sku: Sku) -> Result<PurchaseResponse, SdkError> {
        let res = self
            .http
            .post(format!("{}/purchase", self.base))
            .json(&PurchaseRequest { uid, sku })
            .send()
            .await;
        read(res).await
    }

    /// `GET /orders/{id}`: where the order is right now.
    ///
    /// # Errors
    /// [`SdkError::Server`] with status 404 for an order this server never made;
    /// otherwise as for [`ServerClient::catalog`].
    pub async fn order_status(&self, order_id: OrderId) -> Result<OrderStatus, SdkError> {
        let res = self
            .http
            .get(format!("{}/orders/{order_id}", self.base))
            .send()
            .await;
        read(res).await
    }

    /// Polls the order every [`Config::poll_interval`](crate::Config) until it
    /// is final, then returns that state. Call this after a
    /// [`PurchaseResponse::PendingPayment`] while the checkout is open.
    ///
    /// # Errors
    /// [`SdkError::PollTimeout`] after [`Config::poll_timeout`](crate::Config);
    /// otherwise the first error from [`ServerClient::order_status`].
    pub async fn wait_until_final(&self, order_id: OrderId) -> Result<OrderState, SdkError> {
        let deadline = tokio::time::Instant::now() + self.poll_timeout;
        loop {
            let status = self.order_status(order_id).await?;
            if status.state.is_final() {
                return Ok(status.state);
            }
            if tokio::time::Instant::now() + self.poll_interval > deadline {
                return Err(SdkError::PollTimeout(order_id));
            }
            tracing::debug!(%order_id, ?status.state, "order not final yet");
            tokio::time::sleep(self.poll_interval).await;
        }
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

    fn gold() -> anyhow::Result<CardUid> {
        Ok("04A3B2C1".parse()?)
    }

    #[tokio::test]
    async fn gold_buying_gems_is_approved() -> anyhow::Result<()> {
        let client = ServerClient::new(&demo_server().await?)?;
        let answer = client.purchase(gold()?, Sku::new("gems_500")).await?;
        assert!(
            matches!(answer, PurchaseResponse::Approved { .. }),
            "{answer:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_decline_is_an_answer_not_an_error() -> anyhow::Result<()> {
        let client = ServerClient::new(&demo_server().await?)?;
        let answer = client.purchase(gold()?, Sku::new("not_a_sku")).await?;
        assert_eq!(
            answer,
            PurchaseResponse::Declined {
                reason: tappad_protocol::DeclineReason::UnknownSku
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn an_approved_order_is_final_at_once() -> anyhow::Result<()> {
        let client = ServerClient::new(&demo_server().await?)?;
        let PurchaseResponse::Approved { order_id, .. } =
            client.purchase(gold()?, Sku::new("gems_100")).await?
        else {
            anyhow::bail!("mock must approve");
        };
        assert!(client.order_status(order_id).await?.state.is_success());
        assert!(client.wait_until_final(order_id).await?.is_success());
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_order_is_404() -> anyhow::Result<()> {
        let client = ServerClient::new(&demo_server().await?)?;
        match client.order_status(OrderId(999_999)).await {
            Err(SdkError::Server { status: 404, .. }) => Ok(()),
            other => anyhow::bail!("expected 404, got {other:?}"),
        }
    }

    /// A server whose only order is stuck in `new` forever.
    async fn stuck_server() -> anyhow::Result<Config> {
        use axum::routing::get;
        use axum::Json;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let app = axum::Router::new().route(
            "/orders/{id}",
            get(|| async {
                Json(OrderStatus {
                    order_id: OrderId(1),
                    state: OrderState::New,
                })
            }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await });
        Ok(Config {
            server_url: format!("http://{addr}"),
            poll_interval: Duration::from_millis(5),
            poll_timeout: Duration::from_millis(40),
            ..Config::default()
        })
    }

    /// A server whose provider is down: every purchase is 502 with an `ErrorBody`.
    async fn broken_provider_server() -> anyhow::Result<Config> {
        use axum::http::StatusCode;
        use axum::routing::post;
        use axum::Json;
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let app = axum::Router::new().route(
            "/purchase",
            post(|| async {
                (
                    StatusCode::BAD_GATEWAY,
                    Json(ErrorBody {
                        error: "provider failed".into(),
                    }),
                )
            }),
        );
        tokio::spawn(async move { axum::serve(listener, app).await });
        Ok(Config {
            server_url: format!("http://{addr}"),
            ..Config::default()
        })
    }

    #[tokio::test]
    async fn a_provider_failure_is_a_502_with_the_server_text() -> anyhow::Result<()> {
        let client = ServerClient::new(&broken_provider_server().await?)?;
        match client.purchase(gold()?, Sku::new("gems_500")).await {
            Err(SdkError::Server {
                status: 502,
                message,
            }) => {
                assert_eq!(message, "provider failed");
                Ok(())
            }
            other => anyhow::bail!("expected 502, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_stuck_order_times_out_with_its_id() -> anyhow::Result<()> {
        let client = ServerClient::new(&stuck_server().await?)?;
        match client.wait_until_final(OrderId(1)).await {
            Err(SdkError::PollTimeout(OrderId(1))) => Ok(()),
            other => anyhow::bail!("expected a poll timeout, got {other:?}"),
        }
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
