//! Who may tap and what they may buy. Checked before any network call.

use std::collections::HashMap;

use crate::types::{CardUid, Cents, DeclineReason, Sku};

/// A registered card and its per-tap limit.
#[derive(Debug, Clone)]
pub struct Card {
    /// Player name shown in logs and receipts.
    pub owner: String,
    /// Most a single tap may spend.
    pub limit: Cents,
}

/// A purchasable item.
#[derive(Debug, Clone)]
pub struct Item {
    /// Price in cents.
    pub price: Cents,
    /// Gems granted when paid.
    pub gems: u32,
}

/// Cards and catalogue for the demo, fixed in code.
#[derive(Debug, Clone)]
pub struct Registry {
    cards: HashMap<CardUid, Card>,
    items: HashMap<Sku, Item>,
}

/// A purchase that passed every registry check.
#[derive(Debug, Clone)]
pub struct Cleared {
    /// Who is paying.
    pub owner: String,
    /// What they buy.
    pub sku: Sku,
    /// What it costs.
    pub price: Cents,
}

impl Registry {
    /// Two cards and three items, matching the demo script and the shop page.
    #[must_use]
    pub fn demo() -> Self {
        let card = |owner: &str, limit| Card {
            owner: owner.to_owned(),
            limit: Cents(limit),
        };
        let item = |price, gems| Item {
            price: Cents(price),
            gems,
        };
        Self {
            cards: [
                (CardUid::parse("04A3B2C1"), card("Dad", 5_000)),
                (CardUid::parse("04D4E5F6"), card("Kid", 100)),
            ]
            .into_iter()
            .filter_map(|(uid, card)| uid.map(|uid| (uid, card)))
            .collect(),
            items: [
                ("gems_100", item(99, 100)),
                ("gems_500", item(499, 500)),
                ("gems_1200", item(999, 1200)),
            ]
            .into_iter()
            .map(|(sku, item)| (Sku(sku.to_owned()), item))
            .collect(),
        }
    }

    /// Applies the business rules.
    ///
    /// # Errors
    /// `Err` is a decline reason for the player, not a failure.
    pub fn clear(&self, raw_uid: &str, sku: &Sku) -> Result<Cleared, DeclineReason> {
        let uid = CardUid::parse(raw_uid).ok_or(DeclineReason::UnknownCard)?;
        let card = self.cards.get(&uid).ok_or(DeclineReason::UnknownCard)?;
        let item = self.items.get(sku).ok_or(DeclineReason::UnknownSku)?;
        if item.price > card.limit {
            return Err(DeclineReason::LimitExceeded);
        }
        Ok(Cleared {
            owner: card.owner.clone(),
            sku: sku.clone(),
            price: item.price,
        })
    }

    /// Gems an item grants, if it exists.
    #[must_use]
    pub fn gems_for(&self, sku: &Sku) -> Option<u32> {
        self.items.get(sku).map(|item| item.gems)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sku(s: &str) -> Sku {
        Sku(s.to_owned())
    }

    #[test]
    fn dad_within_limit_is_cleared() {
        let cleared = Registry::demo().clear("04a3b2c1", &sku("gems_500"));
        assert!(matches!(
            cleared,
            Ok(Cleared {
                price: Cents(499),
                ..
            })
        ));
    }

    #[test]
    fn kid_over_limit_is_declined_without_error() {
        let result = Registry::demo().clear("04D4E5F6", &sku("gems_500"));
        assert_eq!(result.err(), Some(DeclineReason::LimitExceeded));
    }

    #[test]
    fn unknown_card_and_sku() {
        let registry = Registry::demo();
        assert_eq!(
            registry.clear("FFFFFFFF", &sku("gems_100")).err(),
            Some(DeclineReason::UnknownCard)
        );
        assert_eq!(
            registry.clear("04A3B2C1", &sku("sword")).err(),
            Some(DeclineReason::UnknownSku)
        );
    }
}
