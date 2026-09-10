//! The one handle a game holds: the pad on one side, the server on the other.

use tappad_protocol::{
    CardUid, CatalogItem, DeclineReason, OrderId, OrderState, PadEvent, PurchaseResponse, Sku,
};

use crate::{Config, Pad, SdkError, ServerClient};

/// How a purchase ended once any checkout the player had to complete is over.
/// The answer to [`TapPad::buy_and_settle`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Paid. Grant the item.
    Granted {
        /// The order that was paid.
        order_id: OrderId,
    },
    /// The registry said no before anything was created. Show [`DeclineReason::message`].
    Declined {
        /// Why.
        reason: DeclineReason,
    },
    /// An order was created but the player did not pay it.
    NotPaid {
        /// The order that ended unpaid.
        order_id: OrderId,
        /// `Canceled` or `Expired`.
        state: OrderState,
    },
}

/// Everything a game needs to sell through `TapPad`.
///
/// ```no_run
/// # async fn demo() -> Result<(), tappad_sdk::SdkError> {
/// use tappad_sdk::{Config, PurchaseResponse, Sku, TapPad};
///
/// let mut tappad = TapPad::connect(&Config::default())?;
/// let uid = tappad.next_tap().await;
/// match tappad.buy(uid, Sku::new("gems_500")).await? {
///     PurchaseResponse::Approved { .. } => { /* grant the gems */ }
///     PurchaseResponse::Declined { reason } => { /* show reason.message() */ }
///     PurchaseResponse::PendingPayment { order_id, checkout_url } => {
///         // open checkout_url in the game window, then:
///         if tappad.wait_for_payment(order_id).await?.is_success() { /* grant */ }
///     }
/// }
/// # Ok(()) }
/// ```
#[derive(Debug)]
pub struct TapPad {
    server: ServerClient,
    pad: Pad,
}

impl TapPad {
    /// Builds the server client and points at the bridge. No network yet.
    ///
    /// # Errors
    /// The HTTP client could not be built.
    pub fn connect(config: &Config) -> Result<Self, SdkError> {
        Ok(TapPad {
            server: ServerClient::new(config)?,
            pad: Pad::new(config),
        })
    }

    /// What the shop can sell. See [`ServerClient::catalog`].
    ///
    /// # Errors
    /// As [`ServerClient::catalog`].
    pub async fn catalog(&self) -> Result<Vec<CatalogItem>, SdkError> {
        self.server.catalog().await
    }

    /// Waits for a card on the pad. See [`Pad::next_tap`].
    pub async fn next_tap(&mut self) -> CardUid {
        self.pad.next_tap().await
    }

    /// Waits for any pad event, so a game can show "pad ready" or a reader
    /// error instead of only reacting to taps. See [`Pad::next_event`].
    pub async fn next_event(&mut self) -> PadEvent {
        self.pad.next_event().await
    }

    /// Buys `sku` with the tapped card. See [`ServerClient::purchase`].
    ///
    /// # Errors
    /// As [`ServerClient::purchase`].
    pub async fn buy(&self, uid: CardUid, sku: Sku) -> Result<PurchaseResponse, SdkError> {
        self.server.purchase(uid, sku).await
    }

    /// Follows a pending order to its final state. See [`ServerClient::wait_until_final`].
    ///
    /// # Errors
    /// As [`ServerClient::wait_until_final`].
    pub async fn wait_for_payment(&self, order_id: OrderId) -> Result<OrderState, SdkError> {
        self.server.wait_until_final(order_id).await
    }

    /// [`TapPad::buy`] and [`TapPad::wait_for_payment`] in one call. When the
    /// server answers with a checkout, `open_checkout` gets the URL to show the
    /// player, then this waits for the order to settle.
    ///
    /// # Errors
    /// As [`ServerClient::purchase`] and [`ServerClient::wait_until_final`].
    pub async fn buy_and_settle(
        &self,
        uid: CardUid,
        sku: Sku,
        open_checkout: impl FnOnce(&str),
    ) -> Result<Outcome, SdkError> {
        match self.buy(uid, sku).await? {
            PurchaseResponse::Approved { order_id, .. } => Ok(Outcome::Granted { order_id }),
            PurchaseResponse::Declined { reason } => Ok(Outcome::Declined { reason }),
            PurchaseResponse::PendingPayment {
                order_id,
                checkout_url,
            } => {
                open_checkout(&checkout_url);
                let state = self.wait_for_payment(order_id).await?;
                if state.is_success() {
                    Ok(Outcome::Granted { order_id })
                } else {
                    Ok(Outcome::NotPaid { order_id, state })
                }
            }
        }
    }

    /// The HTTP side on its own, for a game that drives taps some other way.
    #[must_use]
    pub fn server(&self) -> &ServerClient {
        &self.server
    }

    /// The pad side on its own, for a game that wants `ready` and `error` events too.
    pub fn pad(&mut self) -> &mut Pad {
        &mut self.pad
    }
}

#[cfg(test)]
mod tests {
    use axum::routing::{get, post};
    use axum::Json;
    use tappad_protocol::OrderStatus;
    use tokio::net::TcpListener;

    use super::*;

    /// A server that hands out one checkout and reports it in `state` afterwards.
    async fn checkout_server(state: OrderState) -> anyhow::Result<Config> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let app = axum::Router::new()
            .route(
                "/purchase",
                post(|| async {
                    Json(PurchaseResponse::PendingPayment {
                        order_id: OrderId(42),
                        checkout_url: "https://sandbox-secure.xsolla.com/paystation4/?token=t"
                            .into(),
                    })
                }),
            )
            .route(
                "/orders/{id}",
                get(move || async move {
                    Json(OrderStatus {
                        order_id: OrderId(42),
                        state,
                    })
                }),
            );
        tokio::spawn(async move { axum::serve(listener, app).await });
        Ok(Config {
            server_url: format!("http://{addr}"),
            ..Config::default()
        })
    }

    fn gold() -> anyhow::Result<CardUid> {
        Ok("04A3B2C1".parse()?)
    }

    #[tokio::test]
    async fn a_paid_checkout_is_granted_and_the_url_was_shown() -> anyhow::Result<()> {
        let tappad = TapPad::connect(&checkout_server(OrderState::Paid).await?)?;
        let mut shown = None;
        let outcome = tappad
            .buy_and_settle(gold()?, Sku::new("gems_500"), |url| {
                shown = Some(url.to_owned());
            })
            .await?;
        assert_eq!(
            outcome,
            Outcome::Granted {
                order_id: OrderId(42)
            }
        );
        assert!(
            shown.is_some_and(|u| u.contains("paystation")),
            "checkout not shown"
        );
        Ok(())
    }

    #[tokio::test]
    async fn a_closed_checkout_is_not_paid() -> anyhow::Result<()> {
        let tappad = TapPad::connect(&checkout_server(OrderState::Canceled).await?)?;
        let outcome = tappad
            .buy_and_settle(gold()?, Sku::new("gems_500"), |_| {})
            .await?;
        assert_eq!(
            outcome,
            Outcome::NotPaid {
                order_id: OrderId(42),
                state: OrderState::Canceled
            }
        );
        Ok(())
    }
}
