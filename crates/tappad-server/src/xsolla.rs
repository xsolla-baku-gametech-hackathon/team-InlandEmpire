//! Xsolla Store API as a [`PaymentProvider`]. Two calls, both documented in `docs/xsolla.md`.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

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

/// One order's payment token and when it was stored.
struct Remembered {
    token: String,
    /// The card's per-tap limit, so the amount the store actually charges can be
    /// checked against it when the order is polled.
    limit: Cents,
    at: Instant,
}

/// Xsolla-backed provider. Remembers the payment token of every order it created,
/// because order status is read with that token as Bearer.
///
/// The store is in memory only: restarting the server forgets every order, and
/// `GET /orders/{id}` then answers 404 for one created before the restart.
pub struct XsollaProvider {
    http: reqwest::Client,
    config: XsollaConfig,
    tokens: Mutex<HashMap<OrderId, Remembered>>,
}

/// How long to wait for the TCP and TLS handshake with the store.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// How long one whole store call may take. A stalled store must not park
/// `POST /purchase` forever; the route turns a timeout into HTTP 502.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// How long a payment token is worth keeping. The game stops polling at a final
/// state long before this; anything older is a checkout nobody finished.
const TOKEN_TTL: Duration = Duration::from_secs(2 * 60 * 60);

/// Ceiling on remembered orders, so a long run cannot grow without bound.
const MAX_TOKENS: usize = 10_000;

/// Drops tokens older than [`TOKEN_TTL`], then the oldest ones if still over
/// [`MAX_TOKENS`].
fn evict(tokens: &mut HashMap<OrderId, Remembered>) {
    let now = Instant::now();
    tokens.retain(|_, r| now.duration_since(r.at) < TOKEN_TTL);
    if tokens.len() <= MAX_TOKENS {
        return;
    }
    let mut ages: Vec<(OrderId, Instant)> = tokens.iter().map(|(id, r)| (*id, r.at)).collect();
    ages.sort_by_key(|(_, at)| *at);
    for (id, _) in ages.into_iter().take(tokens.len() - MAX_TOKENS) {
        tokens.remove(&id);
    }
}

impl XsollaProvider {
    /// Builds a client for one project.
    ///
    /// # Errors
    /// The HTTP client cannot be built, for example when TLS fails to initialise.
    pub fn new(config: XsollaConfig) -> Result<Self, ProviderError> {
        Self::build(config, CONNECT_TIMEOUT, REQUEST_TIMEOUT)
    }

    /// [`XsollaProvider::new`] with the timeouts spelled out, so a test can use short ones.
    fn build(
        config: XsollaConfig,
        connect: Duration,
        request: Duration,
    ) -> Result<Self, ProviderError> {
        let http = reqwest::Client::builder()
            .connect_timeout(connect)
            .timeout(request)
            .user_agent(concat!("tappad-server/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| ProviderError::Transport(format!("cannot build http client: {e}")))?;
        Ok(Self {
            http,
            config,
            tokens: Mutex::new(HashMap::new()),
        })
    }

    /// Takes the token lock, recovering from a poisoned one. A panic while a
    /// `HashMap` insert is in flight cannot leave it inconsistent, and dropping
    /// every token because one unrelated task panicked would end the demo.
    fn tokens(&self) -> std::sync::MutexGuard<'_, HashMap<OrderId, Remembered>> {
        self.tokens.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn remember(&self, order_id: OrderId, token: String, limit: Cents) {
        let mut tokens = self.tokens();
        tokens.insert(
            order_id,
            Remembered {
                token,
                limit,
                at: Instant::now(),
            },
        );
        evict(&mut tokens);
    }

    /// The payment token and the card limit stored with it.
    fn remembered(&self, order_id: OrderId) -> Option<(String, Cents)> {
        self.tokens()
            .get(&order_id)
            .map(|r| (r.token.clone(), r.limit))
    }
}

/// Body of `POST .../admin/payment/token`, exactly as `docs/xsolla.md` shows it.
#[must_use]
pub fn token_request(purchase: &Cleared, sandbox: bool) -> serde_json::Value {
    serde_json::json!({
        "sandbox": sandbox,
        "user": {
            "id": { "value": purchase.player_id },
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

/// Answer to the order call. The amount is optional because `docs/xsolla.md` does
/// not pin this part of the shape; when it is absent the amount check is skipped.
#[derive(Debug, Deserialize)]
struct OrderResponse {
    status: String,
    #[serde(default)]
    content: Option<OrderContent>,
}

#[derive(Debug, Deserialize)]
struct OrderContent {
    #[serde(default)]
    price: Option<StorePrice>,
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
    whole
        .checked_mul(100)
        .and_then(|w| w.checked_add(frac))
        .map(Cents)
        .ok_or_else(bad)
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

/// Refuses an order the store priced above the card's per-tap limit.
///
/// The registry checks the local catalogue price before the order is created, but
/// the token request sends only a SKU, so the store charges whatever its own
/// catalogue says. This is the second check, on the amount that was really billed.
///
/// # Errors
/// The amount does not parse, or it is over the limit.
fn check_amount(
    order_id: OrderId,
    content: Option<&OrderContent>,
    limit: Cents,
) -> Result<(), ProviderError> {
    let Some(amount) = content.and_then(|c| c.price.as_ref()) else {
        return Ok(());
    };
    let charged = parse_amount(&amount.amount)?;
    if charged > limit {
        tracing::error!(
            ?order_id,
            %charged,
            %limit,
            "the store charged more than this card may spend; refusing to report it paid"
        );
        return Err(ProviderError::Rejected(
            "order amount is over the card limit".into(),
        ));
    }
    Ok(())
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

/// How long one headless checkout may take before it is given up on. The script
/// drives a real browser through a payment form; 45 seconds is normal.
const AUTOPAY_TIMEOUT: Duration = Duration::from_secs(120);

/// How many headless browsers may run at once. Each one is a Chromium; a queue of
/// taps must not turn into a queue of browsers.
static AUTOPAY_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);

/// Exit code Windows gives for the Microsoft Store `python3` alias, which spawns
/// happily and then does nothing. Any other failure is the script's own.
const WINDOWS_STORE_ALIAS: Option<i32> = Some(9009);

/// Pays a sandbox order headless with `scripts/autopay.py`, relative to the working
/// directory. Tries `python3` then `python` so the same command works on Windows,
/// moving on only when the interpreter itself did not run.
async fn autopay(order_id: OrderId, checkout_url: String) {
    let Ok(_slot) = AUTOPAY_SLOTS.acquire().await else {
        tracing::error!(?order_id, "autopay queue closed");
        return;
    };
    let started = std::time::Instant::now();
    let mut ran = None;
    for python in ["python3", "python"] {
        let call = tokio::process::Command::new(python)
            .arg("scripts/autopay.py")
            .arg(&checkout_url)
            .output();
        match tokio::time::timeout(AUTOPAY_TIMEOUT, call).await {
            Err(_) => {
                tracing::error!(
                    ?order_id,
                    python,
                    "autopay timed out after {AUTOPAY_TIMEOUT:?}"
                );
                return;
            }
            Ok(Err(e)) => tracing::debug!(?order_id, python, "autopay spawn failed: {e}"),
            Ok(Ok(out)) if out.status.code() == WINDOWS_STORE_ALIAS => {
                tracing::debug!(
                    ?order_id,
                    python,
                    "this is the Windows Store alias, trying the next"
                );
            }
            Ok(Ok(out)) => {
                ran = Some((python, out));
                break;
            }
        }
    }
    let Some((python, out)) = ran else {
        tracing::error!(?order_id, "autopay needs python3 or python on PATH");
        return;
    };
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let secs = started.elapsed().as_secs();
    if out.status.success() {
        tracing::info!(?order_id, python, secs, "autopay done: {}", stdout.trim());
    } else {
        tracing::error!(
            ?order_id,
            python,
            secs,
            "autopay failed: {} {}",
            stdout.trim(),
            stderr.trim()
        );
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
        self.remember(order_id, created.token, purchase.limit);
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
        let (token, limit) = self
            .remembered(order_id)
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
        check_amount(order_id, order.content.as_ref(), limit)?;
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
    fn an_amount_too_large_for_cents_is_rejected_not_wrapped() {
        // u64::MAX is 18446744073709551615, so the cents of these do not fit.
        assert!(parse_amount("184467440737095516.16").is_err());
        assert!(parse_amount("18446744073709551615").is_err());
        assert!(parse_amount("99999999999999999999999").is_err());
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
            player_id: "gold-1".into(),
            sku: Sku::new("gems_500"),
            price: Cents(499),
            limit: Cents(5_000),
        }
    }

    #[test]
    fn request_body_matches_doc() {
        let body = token_request(&purchase(), true);
        assert_eq!(body["sandbox"], true);
        assert_eq!(
            body["user"]["id"]["value"], "gold-1",
            "the provider account must be the stable id, not the display name"
        );
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
        })?;
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

    #[test]
    fn eviction_drops_stale_tokens_and_caps_the_map() -> anyhow::Result<()> {
        let mut tokens = HashMap::new();
        let stale = Instant::now()
            .checked_sub(TOKEN_TTL + Duration::from_secs(1))
            .ok_or_else(|| anyhow::anyhow!("this machine booted less than the TTL ago"))?;
        tokens.insert(
            OrderId(1),
            Remembered {
                token: "old".into(),
                limit: Cents(5_000),
                at: stale,
            },
        );
        tokens.insert(
            OrderId(2),
            Remembered {
                token: "fresh".into(),
                limit: Cents(5_000),
                at: Instant::now(),
            },
        );
        evict(&mut tokens);
        assert!(!tokens.contains_key(&OrderId(1)), "a stale token must go");
        assert!(tokens.contains_key(&OrderId(2)), "a fresh token must stay");

        let base = Instant::now();
        let over = u64::try_from(MAX_TOKENS)
            .unwrap_or(u64::MAX)
            .saturating_add(10);
        for i in 0..over {
            tokens.insert(
                OrderId(1000 + i),
                Remembered {
                    token: format!("t{i}"),
                    limit: Cents(5_000),
                    at: base + Duration::from_millis(i),
                },
            );
        }
        evict(&mut tokens);
        assert_eq!(tokens.len(), MAX_TOKENS, "the map must stay bounded");
        Ok(())
    }

    #[test]
    fn an_order_priced_over_the_card_limit_is_refused() {
        let over = OrderContent {
            price: Some(StorePrice {
                amount: "99.00".into(),
                currency: "USD".into(),
            }),
        };
        let err = check_amount(OrderId(1), Some(&over), Cents(5_000));
        assert!(
            matches!(err, Err(ProviderError::Rejected(_))),
            "9900 cents is over a 5000 cent limit, got {err:?}"
        );
    }

    #[test]
    fn an_order_within_the_limit_or_without_an_amount_passes() -> Result<(), ProviderError> {
        let within = OrderContent {
            price: Some(StorePrice {
                amount: "4.99".into(),
                currency: "USD".into(),
            }),
        };
        check_amount(OrderId(1), Some(&within), Cents(5_000))?;
        check_amount(OrderId(1), None, Cents(5_000))?;
        check_amount(OrderId(1), Some(&OrderContent { price: None }), Cents(1))?;
        Ok(())
    }

    /// A listener that accepts and then never answers, standing in for a hung store.
    async fn stalled_store() -> anyhow::Result<String> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((socket, _)) = listener.accept().await {
                held.push(socket);
            }
        });
        Ok(format!("http://{addr}"))
    }

    #[tokio::test]
    async fn a_stalled_store_times_out_as_a_transport_error() -> anyhow::Result<()> {
        let provider = XsollaProvider::build(
            XsollaConfig {
                project_id: "42".into(),
                api_key: SecretString::from("secret"),
                sandbox: true,
                store_url: stalled_store().await?,
                autopay: false,
            },
            Duration::from_millis(200),
            Duration::from_millis(200),
        )?;
        let started = std::time::Instant::now();
        let err = provider.create_order(&purchase()).await;
        assert!(
            matches!(err, Err(ProviderError::Transport(_))),
            "expected a transport error, got {err:?}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the call should give up quickly, took {:?}",
            started.elapsed()
        );
        Ok(())
    }
}
