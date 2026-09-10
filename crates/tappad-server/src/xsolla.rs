//! Xsolla Store API as a [`PaymentProvider`]. Two calls, both documented in `docs/xsolla.md`.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;

use crate::provider::{CreatedOrder, PaymentProvider, ProviderError};
use crate::registry::Cleared;
use crate::types::{CatalogItem, Cents, OrderId, OrderState, Sku};

/// Pay Station theme id for the embedded layout, from the Xsolla docs.
const EMBED_THEME: &str = "63295aab2e47fab76f7708e3";

/// What the client needs to talk to one Xsolla project.
#[derive(Clone)]
pub struct XsollaConfig {
    /// Project id from the Publisher Account URL.
    pub project_id: String,
    /// Server API key. Never logged, never leaves this process.
    pub api_key: SecretString,
    /// `true` creates sandbox orders and points the checkout at the sandbox host.
    pub sandbox: bool,
    /// Store API origin. Production is `https://store.xsolla.com`; tests point elsewhere.
    pub store_url: String,
    /// `true` pays every sandbox order headless through `scripts/autopay.py`,
    /// so a tap completes without a click. Stands in for Xsolla Tokenization.
    pub autopay: bool,
}

impl XsollaConfig {
    /// Reads `XSOLLA_PROJECT_ID`, `XSOLLA_API_KEY`, `XSOLLA_SANDBOX` and `TAPPAD_AUTOPAY`
    /// from the environment.
    ///
    /// # Errors
    /// Missing project id or API key, or `TAPPAD_AUTOPAY=true` outside the sandbox.
    pub fn from_env() -> anyhow::Result<Self> {
        let project_id = std::env::var("XSOLLA_PROJECT_ID")?;
        let api_key = SecretString::from(std::env::var("XSOLLA_API_KEY")?);
        let sandbox = std::env::var("XSOLLA_SANDBOX").map_or(true, |v| v != "false");
        let autopay = std::env::var("TAPPAD_AUTOPAY").is_ok_and(|v| v == "true");
        anyhow::ensure!(
            sandbox || !autopay,
            "TAPPAD_AUTOPAY=true only works with XSOLLA_SANDBOX=true"
        );
        Ok(Self {
            project_id,
            api_key,
            sandbox,
            store_url: "https://store.xsolla.com".into(),
            autopay,
        })
    }

    fn checkout_url(&self, token: &str) -> String {
        let host = if self.sandbox {
            "sandbox-secure.xsolla.com"
        } else {
            "secure.xsolla.com"
        };
        format!("https://{host}/paystation4/?token={token}")
    }
}

/// Xsolla-backed provider. Remembers the payment token of every order it created,
/// because order status is read with that token as Bearer.
pub struct XsollaProvider {
    http: reqwest::Client,
    config: XsollaConfig,
    tokens: Mutex<HashMap<OrderId, String>>,
}

impl XsollaProvider {
    /// Builds a client for one project.
    #[must_use]
    pub fn new(config: XsollaConfig) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
            tokens: Mutex::new(HashMap::new()),
        }
    }

    fn remember(&self, order_id: OrderId, token: String) {
        if let Ok(mut tokens) = self.tokens.lock() {
            tokens.insert(order_id, token);
        }
    }

    fn token_for(&self, order_id: OrderId) -> Option<String> {
        self.tokens.lock().ok()?.get(&order_id).cloned()
    }
}

/// Body of `POST .../admin/payment/token`, exactly as `docs/xsolla.md` shows it.
#[must_use]
pub fn token_request(purchase: &Cleared, sandbox: bool) -> serde_json::Value {
    serde_json::json!({
        "sandbox": sandbox,
        "user": {
            "id": { "value": purchase.owner.to_lowercase() },
            "country": { "value": "US", "allow_modify": false }
        },
        "purchase": { "items": [ { "sku": purchase.sku.as_str(), "quantity": 1 } ] },
        "settings": { "ui": { "layout": "embed", "theme": EMBED_THEME } }
    })
}

/// `201` answer to the token call.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: String,
    order_id: u64,
}

/// Answer to the order call. Only the field we act on.
#[derive(Debug, Deserialize)]
struct OrderResponse {
    status: String,
}

/// Answer to the public catalogue call. Only the fields the shop shows.
#[derive(Debug, Deserialize)]
struct ItemsResponse {
    items: Vec<StoreItem>,
}

#[derive(Debug, Deserialize)]
struct StoreItem {
    sku: String,
    name: String,
    #[serde(default)]
    description: String,
    image_url: Option<String>,
    price: Option<StorePrice>,
    #[serde(default)]
    can_be_bought: bool,
}

#[derive(Debug, Deserialize)]
struct StorePrice {
    amount: String,
    currency: String,
}

/// `"4.99"` to 499 cents. Xsolla sends amounts as decimal strings.
///
/// # Errors
/// Anything that is not digits with at most one dot and two decimals.
pub fn parse_amount(amount: &str) -> Result<Cents, ProviderError> {
    let bad = || ProviderError::Rejected(format!("bad amount {amount:?}"));
    let (whole, frac) = amount.split_once('.').unwrap_or((amount, ""));
    if frac.len() > 2 || whole.is_empty() {
        return Err(bad());
    }
    let whole: u64 = whole.parse().map_err(|_| bad())?;
    let frac: u64 = match frac {
        "" => 0,
        f => format!("{f:0<2}").parse().map_err(|_| bad())?,
    };
    Ok(Cents(whole * 100 + frac))
}

/// Maps the store's item list onto what the shop page renders. Items without a
/// price or not buyable are skipped.
///
/// # Errors
/// A price that does not parse.
pub fn parse_catalog(body: &str) -> Result<Vec<CatalogItem>, ProviderError> {
    let res: ItemsResponse = serde_json::from_str(body)
        .map_err(|e| ProviderError::Rejected(format!("bad items response: {e}")))?;
    res.items
        .into_iter()
        .filter(|i| i.can_be_bought)
        .filter_map(|mut i| i.price.take().map(|p| (i, p)))
        .map(|(i, p)| {
            Ok(CatalogItem {
                sku: Sku::new(&i.sku),
                name: i.name,
                description: i.description,
                price: parse_amount(&p.amount)?,
                currency: p.currency,
                image_url: i.image_url,
            })
        })
        .collect()
}

/// Maps Xsolla's status string onto [`OrderState`].
///
/// # Errors
/// A status the doc does not list.
pub fn parse_order_state(status: &str) -> Result<OrderState, ProviderError> {
    match status {
        "new" => Ok(OrderState::New),
        "paid" => Ok(OrderState::Paid),
        "done" => Ok(OrderState::Done),
        "canceled" | "cancelled" => Ok(OrderState::Canceled),
        "expired" => Ok(OrderState::Expired),
        other => Err(ProviderError::Rejected(format!(
            "unknown order status {other}"
        ))),
    }
}

/// Pays a sandbox order headless with `scripts/autopay.py`, relative to the working
/// directory. Tries `python3` then `python` so the same command works on Windows.
async fn autopay(order_id: OrderId, checkout_url: String) {
    let started = std::time::Instant::now();
    let mut output = None;
    for python in ["python3", "python"] {
        match tokio::process::Command::new(python)
            .arg("scripts/autopay.py")
            .arg(&checkout_url)
            .output()
            .await
        {
            Ok(out) => {
                output = Some(out);
                break;
            }
            Err(e) => tracing::debug!(?order_id, python, "autopay spawn failed: {e}"),
        }
    }
    let Some(out) = output else {
        tracing::error!(?order_id, "autopay needs python3 or python on PATH");
        return;
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let secs = started.elapsed().as_secs();
    if out.status.success() {
        tracing::info!(?order_id, secs, "autopay done: {}", stdout.trim());
    } else {
        tracing::error!(?order_id, secs, "autopay failed: {} {}", stdout.trim(), stderr.trim());
    }
}

async fn failure(res: reqwest::Response) -> ProviderError {
    let status = res.status();
    let body = res.text().await.unwrap_or_default();
    let body: String = body.chars().take(200).collect();
    ProviderError::Rejected(format!("HTTP {status}: {body}"))
}

#[async_trait]
impl PaymentProvider for XsollaProvider {
    async fn create_order(&self, purchase: &Cleared) -> Result<CreatedOrder, ProviderError> {
        let url = format!(
            "{}/api/v3/project/{}/admin/payment/token",
            self.config.store_url, self.config.project_id
        );
        let res = self
            .http
            .post(&url)
            .basic_auth(
                &self.config.project_id,
                Some(self.config.api_key.expose_secret()),
            )
            .json(&token_request(purchase, self.config.sandbox))
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        if !res.status().is_success() {
            return Err(failure(res).await);
        }
        let created: TokenResponse = res
            .json()
            .await
            .map_err(|e| ProviderError::Rejected(format!("bad token response: {e}")))?;
        let order_id = OrderId(created.order_id);
        let checkout_url = self.config.checkout_url(&created.token);
        self.remember(order_id, created.token);
        tracing::info!(owner = %purchase.owner, sku = %purchase.sku, ?order_id, "xsolla order created");
        if self.config.autopay {
            tokio::spawn(autopay(order_id, checkout_url.clone()));
        }
        Ok(CreatedOrder::Pending {
            order_id,
            checkout_url,
        })
    }

    async fn order_state(&self, order_id: OrderId) -> Result<OrderState, ProviderError> {
        let token = self
            .token_for(order_id)
            .ok_or(ProviderError::UnknownOrder(order_id))?;
        let url = format!(
            "{}/api/v2/project/{}/order/{}",
            self.config.store_url, self.config.project_id, order_id.0
        );
        let res = self
            .http
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        if !res.status().is_success() {
            return Err(failure(res).await);
        }
        let order: OrderResponse = res
            .json()
            .await
            .map_err(|e| ProviderError::Rejected(format!("bad order response: {e}")))?;
        parse_order_state(&order.status)
    }

    async fn catalog(&self) -> Result<Vec<CatalogItem>, ProviderError> {
        let url = format!(
            "{}/api/v2/project/{}/items/virtual_items",
            self.config.store_url, self.config.project_id
        );
        let res = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        if !res.status().is_success() {
            return Err(failure(res).await);
        }
        let body = res
            .text()
            .await
            .map_err(|e| ProviderError::Transport(e.to_string()))?;
        parse_catalog(&body)
    }
}

#[cfg(test)]
mod tests {
    use axum::routing::{get, post};
    use axum::{Json, Router};

    use super::*;

    const ITEMS_BODY: &str = r#"{"has_more":false,"items":[
      {"sku":"gems_100","name":"100 gems","description":"100 gems","image_url":null,
       "price":{"amount":"0.99","amount_without_discount":"0.99","currency":"USD"},"can_be_bought":true},
      {"sku":"gems_500","name":"500 gems","description":"500 gems","image_url":"https://cdn/x.png",
       "price":{"amount":"4.99","currency":"USD"},"can_be_bought":true},
      {"sku":"hidden","name":"Hidden","price":{"amount":"1.00","currency":"USD"},"can_be_bought":false},
      {"sku":"free","name":"Free","price":null,"can_be_bought":true}
    ]}"#;

    #[test]
    fn amounts_become_cents() -> Result<(), ProviderError> {
        assert_eq!(parse_amount("0.99")?, Cents(99));
        assert_eq!(parse_amount("4.99")?, Cents(499));
        assert_eq!(parse_amount("12")?, Cents(1200));
        assert_eq!(parse_amount("12.5")?, Cents(1250));
        assert!(parse_amount("1.999").is_err());
        assert!(parse_amount("abc").is_err());
        assert!(parse_amount(".5").is_err());
        Ok(())
    }

    #[test]
    fn catalog_keeps_only_buyable_priced_items() -> Result<(), ProviderError> {
        let items = parse_catalog(ITEMS_BODY)?;
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].sku.as_str(), "gems_100");
        assert_eq!(items[0].price, Cents(99));
        assert_eq!(items[0].currency, "USD");
        assert_eq!(items[0].image_url, None);
        assert_eq!(items[1].image_url.as_deref(), Some("https://cdn/x.png"));
        assert_eq!(items[1].price, Cents(499));
        Ok(())
    }

    fn purchase() -> Cleared {
        Cleared {
            owner: "Gold".into(),
            sku: Sku::new("gems_500"),
            price: Cents(499),
        }
    }

    #[test]
    fn request_body_matches_doc() {
        let body = token_request(&purchase(), true);
        assert_eq!(body["sandbox"], true);
        assert_eq!(body["user"]["id"]["value"], "gold");
        assert_eq!(body["user"]["country"]["value"], "US");
        assert_eq!(body["purchase"]["items"][0]["sku"], "gems_500");
        assert_eq!(body["settings"]["ui"]["layout"], "embed");
    }

    #[test]
    fn status_strings_from_doc() -> Result<(), ProviderError> {
        assert_eq!(parse_order_state("new")?, OrderState::New);
        assert_eq!(parse_order_state("paid")?, OrderState::Paid);
        assert_eq!(parse_order_state("done")?, OrderState::Done);
        assert_eq!(parse_order_state("canceled")?, OrderState::Canceled);
        assert!(parse_order_state("weird").is_err());
        Ok(())
    }

    #[test]
    fn checkout_host_follows_sandbox_flag() {
        let mut config = XsollaConfig {
            project_id: "1".into(),
            api_key: SecretString::from("k"),
            sandbox: true,
            store_url: String::new(),
            autopay: false,
        };
        assert!(config
            .checkout_url("t")
            .starts_with("https://sandbox-secure.xsolla.com/"));
        config.sandbox = false;
        assert!(config
            .checkout_url("t")
            .starts_with("https://secure.xsolla.com/"));
    }

    /// A stand-in Store API on localhost answering the two calls the way the doc shows.
    async fn fake_store() -> anyhow::Result<String> {
        let app = Router::new()
            .route(
                "/api/v3/project/{pid}/admin/payment/token",
                post(|Json(body): Json<serde_json::Value>| async move {
                    assert_eq!(body["purchase"]["items"][0]["sku"], "gems_500");
                    (
                        axum::http::StatusCode::CREATED,
                        Json(serde_json::json!({ "token": "tok123", "order_id": 12345 })),
                    )
                }),
            )
            .route(
                "/api/v2/project/{pid}/order/{oid}",
                get(|headers: axum::http::HeaderMap| async move {
                    let auth = headers
                        .get("authorization")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or_default();
                    assert_eq!(auth, "Bearer tok123");
                    Json(serde_json::json!({ "order_id": 12345, "status": "paid" }))
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            let _ = axum::serve(listener, app).await;
        });
        Ok(format!("http://{addr}"))
    }

    #[tokio::test]
    async fn creates_order_then_polls_with_its_token() -> anyhow::Result<()> {
        let provider = XsollaProvider::new(XsollaConfig {
            project_id: "42".into(),
            api_key: SecretString::from("secret"),
            sandbox: true,
            store_url: fake_store().await?,
            autopay: false,
        });
        let created = provider.create_order(&purchase()).await?;
        let CreatedOrder::Pending {
            order_id,
            checkout_url,
        } = created
        else {
            anyhow::bail!("expected pending, got {created:?}");
        };
        assert_eq!(order_id, OrderId(12345));
        assert_eq!(
            checkout_url,
            "https://sandbox-secure.xsolla.com/paystation4/?token=tok123"
        );
        assert_eq!(provider.order_state(order_id).await?, OrderState::Paid);
        assert!(matches!(
            provider.order_state(OrderId(1)).await,
            Err(ProviderError::UnknownOrder(_))
        ));
        Ok(())
    }
}
