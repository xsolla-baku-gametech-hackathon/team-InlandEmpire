//! HTTP surface: `POST /purchase` and `GET /orders/{id}`.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::provider::{CreatedOrder, PaymentProvider, ProviderError};
use crate::registry::Registry;
use crate::types::{OrderId, OrderStatus, PurchaseRequest, PurchaseResponse};

/// Everything a handler needs.
#[derive(Clone)]
pub struct AppState {
    /// Cards and catalogue.
    pub registry: Arc<Registry>,
    /// The payment backend.
    pub provider: Arc<dyn PaymentProvider>,
}

/// Builds the router.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/purchase", post(purchase))
        .route("/orders/{id}", get(order))
        .with_state(state)
}

/// Provider failure as an HTTP answer.
struct Upstream(ProviderError);

impl axum::response::IntoResponse for Upstream {
    fn into_response(self) -> axum::response::Response {
        let status = match self.0 {
            ProviderError::UnknownOrder(_) => StatusCode::NOT_FOUND,
            ProviderError::Transport(_) | ProviderError::Rejected(_) => StatusCode::BAD_GATEWAY,
        };
        tracing::warn!(error = %self.0, "provider call failed");
        (
            status,
            Json(serde_json::json!({ "error": self.0.to_string() })),
        )
            .into_response()
    }
}

async fn purchase(
    State(state): State<AppState>,
    Json(req): Json<PurchaseRequest>,
) -> Result<Json<PurchaseResponse>, Upstream> {
    let cleared = match state.registry.clear(&req.uid, &req.sku) {
        Ok(cleared) => cleared,
        Err(reason) => {
            tracing::info!(uid = %req.uid, sku = %req.sku, ?reason, "declined");
            return Ok(Json(PurchaseResponse::Declined { reason }));
        }
    };
    let response = match state
        .provider
        .create_order(&cleared)
        .await
        .map_err(Upstream)?
    {
        CreatedOrder::Pending {
            order_id,
            checkout_url,
        } => PurchaseResponse::PendingPayment {
            order_id,
            checkout_url,
        },
        CreatedOrder::Approved {
            order_id,
            receipt_id,
        } => PurchaseResponse::Approved {
            order_id,
            receipt_id,
        },
    };
    Ok(Json(response))
}

async fn order(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<OrderStatus>, Upstream> {
    let order_id = OrderId(id);
    let state = state
        .provider
        .order_state(order_id)
        .await
        .map_err(Upstream)?;
    Ok(Json(OrderStatus { order_id, state }))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{header, Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::provider::MockProvider;
    use crate::types::{DeclineReason, OrderState};

    fn app() -> Router {
        router(AppState {
            registry: Arc::new(Registry::demo()),
            provider: Arc::new(MockProvider::default()),
        })
    }

    async fn post_purchase(
        app: Router,
        body: &str,
    ) -> anyhow::Result<(StatusCode, PurchaseResponse)> {
        let req = Request::post("/purchase")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))?;
        let res = app.oneshot(req).await?;
        let status = res.status();
        let bytes = res.into_body().collect().await?.to_bytes();
        Ok((status, serde_json::from_slice(&bytes)?))
    }

    #[tokio::test]
    async fn dad_is_approved() -> anyhow::Result<()> {
        let (status, body) = post_purchase(app(), r#"{"uid":"04A3B2C1","sku":"gems_500"}"#).await?;
        assert_eq!(status, StatusCode::OK);
        assert!(matches!(body, PurchaseResponse::Approved { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn kid_is_declined_with_200() -> anyhow::Result<()> {
        let (status, body) = post_purchase(app(), r#"{"uid":"04D4E5F6","sku":"gems_500"}"#).await?;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            PurchaseResponse::Declined {
                reason: DeclineReason::LimitExceeded
            }
        );
        Ok(())
    }

    #[tokio::test]
    async fn order_status_after_purchase() -> anyhow::Result<()> {
        let app = app();
        let (_, body) =
            post_purchase(app.clone(), r#"{"uid":"04A3B2C1","sku":"gems_100"}"#).await?;
        let PurchaseResponse::Approved { order_id, .. } = body else {
            anyhow::bail!("expected approved, got {body:?}");
        };
        let req = Request::get(format!("/orders/{}", order_id.0)).body(Body::empty())?;
        let res = app.oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await?.to_bytes();
        let status: OrderStatus = serde_json::from_slice(&bytes)?;
        assert_eq!(status.state, OrderState::Done);
        Ok(())
    }

    #[tokio::test]
    async fn unknown_order_is_404() -> anyhow::Result<()> {
        let req = Request::get("/orders/999").body(Body::empty())?;
        let res = app().oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        Ok(())
    }
}
