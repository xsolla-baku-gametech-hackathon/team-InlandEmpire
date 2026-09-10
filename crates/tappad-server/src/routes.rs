//! HTTP surface: `GET /catalog`, `POST /purchase` and `GET /orders/{id}`.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::provider::{CreatedOrder, PaymentProvider, ProviderError};
use crate::registry::Registry;
use crate::types::{
    CatalogItem, ErrorBody, OrderId, OrderStatus, PurchaseRequest, PurchaseResponse,
};

/// Everything a handler needs.
#[derive(Clone)]
pub struct AppState {
    /// Cards and catalogue.
    pub registry: Arc<Registry>,
    /// The payment backend.
    pub provider: Arc<dyn PaymentProvider>,
}

/// Origins the game is served from: the two Tauri webview origins, and the
/// `python -m http.server` preview in `.claude/launch.json`. `from_static` is
/// const, so a typo here fails the build instead of dropping an origin.
const GAME_ORIGINS: [HeaderValue; 4] = [
    HeaderValue::from_static("tauri://localhost"),
    HeaderValue::from_static("http://tauri.localhost"),
    HeaderValue::from_static("http://localhost:8790"),
    HeaderValue::from_static("http://127.0.0.1:8790"),
];

/// Builds the router.
///
/// The game page lives on another origin (`http://tauri.localhost`, `tauri://localhost`, or the
/// dev preview), so the browser preflights `POST /purchase`. Only those origins are allowed: the
/// server has no authentication, so any page the browser lets through could spend a known card.
pub fn router(state: AppState) -> Router {
    let origins = AllowOrigin::list(GAME_ORIGINS);
    let cors = CorsLayer::new()
        .allow_origin(origins)
        .allow_methods([Method::GET, Method::POST])
        .allow_headers([header::CONTENT_TYPE]);
    Router::new()
        .route("/catalog", get(catalog))
        .route("/purchase", post(purchase))
        .route("/orders/{id}", get(order))
        .fallback(not_found)
        .layer(cors)
        .with_state(state)
}

/// Anything that is not a 2xx. Always serialises as [`ErrorBody`], because that is
/// what `docs/protocol.md` promises for every non-2xx answer.
struct ApiError {
    status: StatusCode,
    /// Text the client may see. Never carries upstream detail; that goes to the log.
    message: &'static str,
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message.to_owned(),
            }),
        )
            .into_response()
    }
}

impl From<ProviderError> for ApiError {
    /// Logs the provider's own words and gives the client a fixed sentence, so a
    /// URL with the project id in it or an upstream body never reaches the page.
    fn from(err: ProviderError) -> Self {
        tracing::warn!(error = %err, "provider call failed");
        match err {
            ProviderError::UnknownOrder(_) => Self {
                status: StatusCode::NOT_FOUND,
                message: "unknown order",
            },
            ProviderError::Transport(_) | ProviderError::Rejected(_) => Self {
                status: StatusCode::BAD_GATEWAY,
                message: "payment provider unavailable",
            },
        }
    }
}

impl From<JsonRejection> for ApiError {
    fn from(err: JsonRejection) -> Self {
        tracing::debug!(error = %err, "bad request body");
        Self {
            status: err.status(),
            message: "request body is not a valid purchase",
        }
    }
}

/// Answer for a path this server does not serve.
async fn not_found() -> ApiError {
    ApiError {
        status: StatusCode::NOT_FOUND,
        message: "no such route",
    }
}

async fn catalog(State(state): State<AppState>) -> Result<Json<Vec<CatalogItem>>, ApiError> {
    let items = state.provider.catalog().await?;
    Ok(Json(items))
}

async fn purchase(
    State(state): State<AppState>,
    body: Result<Json<PurchaseRequest>, JsonRejection>,
) -> Result<Json<PurchaseResponse>, ApiError> {
    let Json(req) = body?;
    let cleared = match state.registry.clear(&req.uid, &req.sku) {
        Ok(cleared) => cleared,
        Err(reason) => {
            tracing::info!(uid = %req.uid, sku = %req.sku, ?reason, "declined");
            return Ok(Json(PurchaseResponse::Declined { reason }));
        }
    };
    let response = match state.provider.create_order(&cleared).await? {
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
) -> Result<Json<OrderStatus>, ApiError> {
    let order_id = OrderId(id);
    let order_state = state.provider.order_state(order_id).await?;
    Ok(Json(OrderStatus {
        order_id,
        state: order_state,
    }))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::provider::MockProvider;
    use crate::types::{DeclineReason, OrderState};

    fn app() -> anyhow::Result<Router> {
        Ok(router(AppState {
            registry: Arc::new(Registry::demo()?),
            provider: Arc::new(MockProvider::default()),
        }))
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
        let (status, body) =
            post_purchase(app()?, r#"{"uid":"04A3B2C1","sku":"gems_500"}"#).await?;
        assert_eq!(status, StatusCode::OK);
        assert!(matches!(body, PurchaseResponse::Approved { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn kid_is_declined_with_200() -> anyhow::Result<()> {
        let (status, body) =
            post_purchase(app()?, r#"{"uid":"04D4E5F6","sku":"gems_500"}"#).await?;
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
        let app = app()?;
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
    async fn preflight_from_the_game_origin_is_allowed() -> anyhow::Result<()> {
        let req = Request::builder()
            .method(Method::OPTIONS)
            .uri("/purchase")
            .header(header::ORIGIN, "http://tauri.localhost")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
            .body(Body::empty())?;
        let res = app()?.oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::OK);
        let allow = res
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .and_then(|v| v.to_str().ok());
        assert_eq!(allow, Some("http://tauri.localhost"));
        let methods = res
            .headers()
            .get(header::ACCESS_CONTROL_ALLOW_METHODS)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        assert!(methods.contains("POST"), "{methods}");
        Ok(())
    }

    #[tokio::test]
    async fn preflight_from_a_foreign_origin_is_refused() -> anyhow::Result<()> {
        let req = Request::builder()
            .method(Method::OPTIONS)
            .uri("/purchase")
            .header(header::ORIGIN, "https://evil.example")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .header(header::ACCESS_CONTROL_REQUEST_HEADERS, "content-type")
            .body(Body::empty())?;
        let res = app()?.oneshot(req).await?;
        assert_eq!(
            res.headers().get(header::ACCESS_CONTROL_ALLOW_ORIGIN),
            None,
            "a foreign origin must not be told it may call this server"
        );
        Ok(())
    }

    #[tokio::test]
    async fn catalog_lists_the_three_gem_packs() -> anyhow::Result<()> {
        let req = Request::get("/catalog").body(Body::empty())?;
        let res = app()?.oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.into_body().collect().await?.to_bytes();
        let items: Vec<CatalogItem> = serde_json::from_slice(&body)?;
        let skus: Vec<&str> = items.iter().map(|i| i.sku.as_str()).collect();
        assert_eq!(skus, ["gems_100", "gems_500", "gems_1200"]);
        Ok(())
    }

    /// A provider that answers however a test needs, so the paths `MockProvider`
    /// never takes (a real pending checkout, an upstream that is down) are covered.
    struct Scripted(Outcome);

    #[derive(Clone, Copy)]
    enum Outcome {
        Pending,
        Transport,
        Rejected,
    }

    #[async_trait::async_trait]
    impl PaymentProvider for Scripted {
        async fn create_order(
            &self,
            _: &crate::registry::Cleared,
        ) -> Result<CreatedOrder, ProviderError> {
            match self.0 {
                Outcome::Pending => Ok(CreatedOrder::Pending {
                    order_id: OrderId(12345),
                    checkout_url: "https://sandbox-secure.xsolla.com/paystation4/?token=tok".into(),
                }),
                Outcome::Transport => Err(ProviderError::Transport(
                    "https://store.xsolla.com/api/v3/project/315338/... connection refused".into(),
                )),
                Outcome::Rejected => Err(ProviderError::Rejected(
                    "HTTP 401: bad key sk_live_abc".into(),
                )),
            }
        }

        async fn order_state(&self, _: OrderId) -> Result<OrderState, ProviderError> {
            Ok(OrderState::New)
        }

        async fn catalog(&self) -> Result<Vec<CatalogItem>, ProviderError> {
            Ok(Vec::new())
        }
    }

    fn scripted_app(outcome: Outcome) -> anyhow::Result<Router> {
        Ok(router(AppState {
            registry: Arc::new(Registry::demo()?),
            provider: Arc::new(Scripted(outcome)),
        }))
    }

    #[tokio::test]
    async fn a_pending_checkout_reaches_the_game_whole() -> anyhow::Result<()> {
        let req = Request::post("/purchase")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"uid":"04A3B2C1","sku":"gems_500"}"#))?;
        let res = scripted_app(Outcome::Pending)?.oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await?.to_bytes();
        let raw: serde_json::Value = serde_json::from_slice(&bytes)?;
        assert_eq!(raw["status"], "pending_payment");
        assert_eq!(raw["order_id"], 12345);
        assert_eq!(
            raw["checkout_url"],
            "https://sandbox-secure.xsolla.com/paystation4/?token=tok"
        );
        let typed: PurchaseResponse = serde_json::from_slice(&bytes)?;
        assert!(matches!(typed, PurchaseResponse::PendingPayment { .. }));
        Ok(())
    }

    #[tokio::test]
    async fn an_upstream_failure_is_502_and_says_nothing_about_upstream() -> anyhow::Result<()> {
        for outcome in [Outcome::Transport, Outcome::Rejected] {
            let req = Request::post("/purchase")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(r#"{"uid":"04A3B2C1","sku":"gems_500"}"#))?;
            let res = scripted_app(outcome)?.oneshot(req).await?;
            assert_eq!(res.status(), StatusCode::BAD_GATEWAY);
            let body = error_body(res).await?;
            assert_eq!(body.error, "payment provider unavailable");
            assert!(
                !body.error.contains("xsolla") && !body.error.contains("sk_live"),
                "the client must not see upstream detail: {}",
                body.error
            );
        }
        Ok(())
    }

    /// Reads the body of a non-2xx answer as the `ErrorBody` the protocol promises.
    async fn error_body(res: axum::response::Response) -> anyhow::Result<ErrorBody> {
        let bytes = res.into_body().collect().await?.to_bytes();
        Ok(serde_json::from_slice(&bytes)?)
    }

    #[tokio::test]
    async fn a_body_that_is_not_a_purchase_is_an_error_body() -> anyhow::Result<()> {
        let req = Request::post("/purchase")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("not json"))?;
        let res = app()?.oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let body = error_body(res).await?;
        assert_eq!(body.error, "request body is not a valid purchase");
        Ok(())
    }

    #[tokio::test]
    async fn a_purchase_missing_a_field_is_an_error_body() -> anyhow::Result<()> {
        let req = Request::post("/purchase")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"uid":"04A3B2C1"}"#))?;
        let res = app()?.oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::UNPROCESSABLE_ENTITY);
        assert!(!error_body(res).await?.error.is_empty());
        Ok(())
    }

    #[tokio::test]
    async fn an_unknown_route_is_an_error_body() -> anyhow::Result<()> {
        let req = Request::get("/nope").body(Body::empty())?;
        let res = app()?.oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_eq!(error_body(res).await?.error, "no such route");
        Ok(())
    }

    #[tokio::test]
    async fn unknown_order_is_404() -> anyhow::Result<()> {
        let req = Request::get("/orders/999").body(Body::empty())?;
        let res = app()?.oneshot(req).await?;
        assert_eq!(res.status(), StatusCode::NOT_FOUND);
        assert_eq!(error_body(res).await?.error, "unknown order");
        Ok(())
    }
}
