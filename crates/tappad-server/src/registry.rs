//! Who may tap and what they may buy. Checked before any network call.

use std::collections::HashMap;

use crate::types::{CardUid, Cents, DeclineReason, Sku, UidError};

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
    /// Demo cards and three items, matching the demo script and the shop page.
    ///
    /// Two fake UIDs, used by `tappad-bridge --fake` and the game's dev buttons, and the two
    /// physical cards read on 2026-09-10 through the real pad on the demo laptop.
    ///
    /// # Errors
    /// A demo UID that does not parse. That is a typo in this file, and it must stop the
    /// server: a card that silently vanished would show up on stage as "not registered".
    pub fn demo() -> Result<Self, UidError> {
        let card = |owner: &str, limit| Card {
            owner: owner.to_owned(),
            limit: Cents(limit),
        };
        let item = |price, gems| Item {
            price: Cents(price),
            gems,
        };
        let mut cards = HashMap::new();
        for (uid, card) in [
            ("04A3B2C1", card("Gold", 5_000)),
            ("04D4E5F6", card("Starter", 100)),
            ("C95DD006", card("Gold", 5_000)),
            ("D9916906", card("Starter", 100)),
        ] {
            cards.insert(uid.parse::<CardUid>()?, card);
        }
        Ok(Self {
            cards,
            items: [
                ("gems_100", item(99, 100)),
                ("gems_500", item(499, 500)),
                ("gems_1200", item(999, 1200)),
            ]
            .into_iter()
            .map(|(sku, item)| (Sku::new(sku), item))
            .collect(),
        })
    }

    /// How many cards are registered.
    #[must_use]
    pub fn card_count(&self) -> usize {
        self.cards.len()
    }

    /// Applies the business rules.
    ///
    /// # Errors
    /// `Err` is a decline reason for the player, not a failure.
    pub fn clear(&self, uid: &CardUid, sku: &Sku) -> Result<Cleared, DeclineReason> {
        let card = self.cards.get(uid).ok_or(DeclineReason::UnknownCard)?;
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
        Sku::new(s)
    }

    fn uid(s: &str) -> Result<CardUid, UidError> {
        s.parse()
    }

    #[test]
    fn dad_within_limit_is_cleared() -> Result<(), UidError> {
        let cleared = Registry::demo()?.clear(&uid("04a3b2c1")?, &sku("gems_500"));
        assert!(matches!(
            cleared,
            Ok(Cleared {
                price: Cents(499),
                ..
            })
        ));
        Ok(())
    }

    #[test]
    fn kid_over_limit_is_declined_without_error() -> Result<(), UidError> {
        let result = Registry::demo()?.clear(&uid("04D4E5F6")?, &sku("gems_500"));
        assert_eq!(result.err(), Some(DeclineReason::LimitExceeded));
        Ok(())
    }

    #[test]
    fn unknown_card_and_sku() -> Result<(), UidError> {
        let registry = Registry::demo()?;
        assert_eq!(
            registry.clear(&uid("FFFFFFFF")?, &sku("gems_100")).err(),
            Some(DeclineReason::UnknownCard)
        );
        assert_eq!(
            registry.clear(&uid("04A3B2C1")?, &sku("sword")).err(),
            Some(DeclineReason::UnknownSku)
        );
        Ok(())
    }

    #[test]
    fn the_demo_registry_keeps_every_card() -> Result<(), UidError> {
        assert_eq!(Registry::demo()?.card_count(), 4);
        Ok(())
    }
}
