//! The one handle a game holds: the pad on one side, the server on the other.

use tappad_protocol::{CardUid, CatalogItem, OrderId, OrderState, PurchaseResponse, Sku};

use crate::{Config, Pad, SdkError, ServerClient};

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
