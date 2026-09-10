//! The one door to a payment provider. Xsolla in production, mock for tests and offline demos.

use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;

use crate::registry::Cleared;
use crate::types::{CatalogItem, Cents, OrderId, OrderState, ReceiptId, Sku};

/// What a provider did with a cleared purchase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreatedOrder {
    /// Order exists, player confirms at this URL.
    Pending {
        /// Provider order id.
        order_id: OrderId,
        /// Checkout page for the game's iframe.
        checkout_url: String,
    },
    /// Paid on the spot, no checkout page.
    Approved {
        /// Provider order id.
        order_id: OrderId,
        /// Receipt reference.
        receipt_id: ReceiptId,
    },
}

/// Provider failures. The route maps [`ProviderError::UnknownOrder`] to HTTP 404
/// and the other two to HTTP 502.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// Network or HTTP failure talking to the provider.
    #[error("provider unreachable: {0}")]
    Transport(String),
    /// Provider answered with something we could not use.
    #[error("provider rejected the request: {0}")]
    Rejected(String),
    /// Order id is not one this provider knows.
    #[error("unknown order {0:?}")]
    UnknownOrder(OrderId),
}

/// A payment backend.
#[async_trait]
pub trait PaymentProvider: Send + Sync {
    /// Creates an order for a purchase that already passed the registry.
    async fn create_order(&self, purchase: &Cleared) -> Result<CreatedOrder, ProviderError>;

    /// Current state of an order this provider created.
    async fn order_state(&self, order_id: OrderId) -> Result<OrderState, ProviderError>;

    /// What the shop page can sell, in store order.
    async fn catalog(&self) -> Result<Vec<CatalogItem>, ProviderError>;
}

/// Approves everything instantly. Used when `TAPPAD_PROVIDER=mock` and in tests.
#[derive(Debug, Default)]
pub struct MockProvider {
    next_order: AtomicU64,
}

#[async_trait]
impl PaymentProvider for MockProvider {
    async fn create_order(&self, purchase: &Cleared) -> Result<CreatedOrder, ProviderError> {
        let order_id = OrderId(self.next_order.fetch_add(1, Ordering::Relaxed) + 1);
        tracing::info!(owner = %purchase.owner, sku = %purchase.sku, price = %purchase.price, ?order_id, "mock approved");
        Ok(CreatedOrder::Approved {
            order_id,
            receipt_id: ReceiptId::new(format!("rcpt-{:06}", order_id.0)),
        })
    }

    async fn order_state(&self, order_id: OrderId) -> Result<OrderState, ProviderError> {
        if order_id.0 == 0 || order_id.0 > self.next_order.load(Ordering::Relaxed) {
            return Err(ProviderError::UnknownOrder(order_id));
        }
        Ok(OrderState::Done)
    }

    async fn catalog(&self) -> Result<Vec<CatalogItem>, ProviderError> {
        let item = |sku: &str, name: &str, price| CatalogItem {
            sku: Sku::new(sku),
            name: name.to_owned(),
            description: name.to_owned(),
            price: Cents(price),
            currency: "USD".into(),
            image_url: None,
        };
        Ok(vec![
            item("gems_100", "100 gems", 99),
            item("gems_500", "500 gems", 499),
            item("gems_1200", "1200 gems", 999),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Cents, Sku};

    fn purchase() -> Cleared {
        Cleared {
            owner: "Gold".into(),
            player_id: "gold-1".into(),
            sku: Sku::new("gems_500"),
            price: Cents(499),
            limit: Cents(5_000),
        }
    }

    #[tokio::test]
    async fn mock_numbers_orders_and_reports_them_done() -> Result<(), ProviderError> {
        let mock = MockProvider::default();
        let first = mock.create_order(&purchase()).await?;
        let CreatedOrder::Approved { order_id, .. } = first else {
            return Err(ProviderError::Rejected("mock should approve".into()));
        };
        assert_eq!(order_id, OrderId(1));
        assert_eq!(mock.order_state(order_id).await?, OrderState::Done);
        assert!(mock.order_state(OrderId(9)).await.is_err());
        Ok(())
    }
}
