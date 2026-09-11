//! Who may tap and what they may buy. Checked before any network call.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use crate::types::{CardUid, Cents, DeclineReason, PurchaseResponse, Sku, UidError};

/// Two taps closer together than this on the same card and item are one purchase.
/// The pad debounces, but a double click on Buy or a retry should not create a
/// second order, and money is involved.
const DOUBLE_TAP_WINDOW: Duration = Duration::from_secs(3);

/// Most a single card may spend in one run of the server, unless
/// `TAPPAD_CARD_CAP_CENTS` says otherwise. The per-tap limit alone lets a card
/// spend without bound in small steps.
pub const DEFAULT_CARD_CAP: Cents = Cents(50_000);

/// Takes one of the spend locks, recovering from poisoning: a panic elsewhere
/// must not turn every later tap into a failure.
fn lock<T>(what: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    what.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A registered card and its per-tap limit.
#[derive(Debug, Clone)]
pub struct Card {
    /// Player name shown in logs and receipts.
    pub owner: String,
    /// Stable id this card pays under at the provider. Not the name, which is for
    /// people, and not the UID, which is the tap credential.
    pub player_id: String,
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

/// Cards and catalogue for the demo, fixed in code, plus what has been spent so
/// far this run. Shared behind an `Arc`, so the spend state lives behind a mutex.
#[derive(Debug)]
pub struct Registry {
    cards: HashMap<CardUid, Card>,
    items: HashMap<Sku, Item>,
    cap: Cents,
    spent: Mutex<HashMap<CardUid, Cents>>,
    recent: Mutex<HashMap<(CardUid, Sku), (Instant, PurchaseResponse)>>,
}

/// A purchase that passed every registry check.
#[derive(Debug, Clone)]
pub struct Cleared {
    /// Who is paying, for logs and receipts.
    pub owner: String,
    /// The provider-side account this purchase belongs to.
    pub player_id: String,
    /// What they buy.
    pub sku: Sku,
    /// What it costs in the local catalogue.
    pub price: Cents,
    /// Most this card may spend on one tap. Carried through so the provider can
    /// check the amount it is actually charging against it.
    pub limit: Cents,
}

impl Registry {
    /// Demo cards and three items, matching the demo script and the shop page.
    ///
    /// Two fake UIDs, used by `tappad-bridge --fake` and the game's dev buttons, and the
    /// physical objects read on 2026-09-10 through the real pad on the demo laptop. Gold, the
    /// white card with the Xsolla sticker, pays for every pack (the dearest is 999 cents).
    /// Blocked, the white card with the All The Things sticker, has a zero limit, so it is
    /// declined on stage as `LimitExceeded`. A phone paying with Apple Pay emits a fresh random UID on
    /// every tap, so it is never in this list and is declined as `UnknownCard`.
    ///
    /// # Errors
    /// A demo UID that does not parse. That is a typo in this file, and it must stop the
    /// server: a card that silently vanished would show up on stage as "not registered".
    pub fn demo() -> Result<Self, UidError> {
        let card = |owner: &str, player_id: &str, limit| Card {
            owner: owner.to_owned(),
            player_id: player_id.to_owned(),
            limit: Cents(limit),
        };
        let item = |price, gems| Item {
            price: Cents(price),
            gems,
        };
        let mut cards = HashMap::new();
        for (uid, card) in [
            ("04A3B2C1", card("Gold", "gold-fake", 5_000)),
            ("04D4E5F6", card("Starter", "starter-fake", 100)),
            ("8FF14EF1", card("Gold", "gold-1", 5_000)),
            ("C95DD006", card("Blocked", "blocked-1", 0)),
            ("D9916906", card("Silver", "silver-1", 1_000)),
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
            cap: DEFAULT_CARD_CAP,
            spent: Mutex::new(HashMap::new()),
            recent: Mutex::new(HashMap::new()),
        })
    }

    /// Sets the per-card spending cap for this run.
    #[must_use]
    pub fn with_cap(mut self, cap: Cents) -> Self {
        self.cap = cap;
        self
    }

    /// How many cards are registered.
    #[must_use]
    pub fn card_count(&self) -> usize {
        self.cards.len()
    }

    /// Applies the business rules: the card exists, the item exists, the price is
    /// within the per-tap limit, and the card has not already spent its cap.
    ///
    /// # Errors
    /// `Err` is a decline reason for the player, not a failure.
    pub fn clear(&self, uid: &CardUid, sku: &Sku) -> Result<Cleared, DeclineReason> {
        let card = self.cards.get(uid).ok_or(DeclineReason::UnknownCard)?;
        let item = self.items.get(sku).ok_or(DeclineReason::UnknownSku)?;
        if item.price > card.limit {
            return Err(DeclineReason::LimitExceeded);
        }
        let spent = self.spent_by(uid);
        let after = spent
            .checked_add(item.price)
            .ok_or(DeclineReason::LimitExceeded)?;
        if after > self.cap {
            tracing::warn!(%spent, price = %item.price, cap = %self.cap, "card is at its cap");
            return Err(DeclineReason::LimitExceeded);
        }
        Ok(Cleared {
            owner: card.owner.clone(),
            player_id: card.player_id.clone(),
            sku: sku.clone(),
            price: item.price,
            limit: card.limit,
        })
    }

    /// What this card has spent so far this run.
    #[must_use]
    pub fn spent_by(&self, uid: &CardUid) -> Cents {
        lock(&self.spent).get(uid).copied().unwrap_or(Cents(0))
    }

    /// Counts a purchase against the card's cap.
    pub fn record_spend(&self, uid: &CardUid, price: Cents) {
        let mut spent = lock(&self.spent);
        let total = spent.entry(uid.clone()).or_insert(Cents(0));
        *total = total.checked_add(price).unwrap_or(*total);
    }

    /// The answer already given for this card and item moments ago, if any. Lets a
    /// double tap return the order that exists instead of creating a second one.
    #[must_use]
    pub fn recent_answer(&self, uid: &CardUid, sku: &Sku) -> Option<PurchaseResponse> {
        let mut recent = lock(&self.recent);
        let now = Instant::now();
        recent.retain(|_, (at, _)| now.duration_since(*at) < DOUBLE_TAP_WINDOW);
        recent
            .get(&(uid.clone(), sku.clone()))
            .map(|(_, response)| response.clone())
    }

    /// Remembers an answer for [`DOUBLE_TAP_WINDOW`].
    pub fn remember_answer(&self, uid: &CardUid, sku: &Sku, response: &PurchaseResponse) {
        lock(&self.recent).insert(
            (uid.clone(), sku.clone()),
            (Instant::now(), response.clone()),
        );
    }

    /// Gems an item grants, if it exists.
    #[must_use]
    pub fn gems_for(&self, sku: &Sku) -> Option<u32> {
        self.items.get(sku).map(|item| item.gems)
    }

    /// What the local catalogue thinks an item costs, if it is one we sell.
    #[must_use]
    pub fn price_for(&self, sku: &Sku) -> Option<Cents> {
        self.items.get(sku).map(|item| item.price)
    }

    /// Every item we sell, for comparing against the store's own catalogue.
    pub fn items(&self) -> impl Iterator<Item = (&Sku, Cents)> {
        self.items.iter().map(|(sku, item)| (sku, item.price))
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
    fn gold_within_limit_is_cleared() -> Result<(), UidError> {
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
    fn starter_over_limit_is_declined_without_error() -> Result<(), UidError> {
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
    fn a_card_cannot_spend_past_its_cap() -> anyhow::Result<()> {
        // Two 4.99 packs is 9.98, so a 6.00 cap stops the second one even though
        // each single purchase is inside the 50.00 per-tap limit.
        let registry = Registry::demo()?.with_cap(Cents(600));
        let gold = uid("04A3B2C1")?;
        let first = registry
            .clear(&gold, &sku("gems_500"))
            .map_err(|reason| anyhow::anyhow!("first purchase should clear, got {reason:?}"))?;
        registry.record_spend(&gold, first.price);
        assert_eq!(registry.spent_by(&gold), Cents(499));
        assert_eq!(
            registry.clear(&gold, &sku("gems_500")).err(),
            Some(DeclineReason::LimitExceeded),
            "the cap must stop a card spending without bound in small steps"
        );
        Ok(())
    }

    #[test]
    fn the_cap_is_per_card() -> anyhow::Result<()> {
        let registry = Registry::demo()?.with_cap(Cents(600));
        registry.record_spend(&uid("04A3B2C1")?, Cents(499));
        assert!(
            registry.clear(&uid("8FF14EF1")?, &sku("gems_500")).is_ok(),
            "another card must be unaffected"
        );
        Ok(())
    }

    #[test]
    fn a_repeat_of_the_same_tap_returns_the_same_answer() -> Result<(), UidError> {
        let registry = Registry::demo()?;
        let gold = uid("04A3B2C1")?;
        let item = sku("gems_500");
        assert!(registry.recent_answer(&gold, &item).is_none());
        let answer = PurchaseResponse::Declined {
            reason: DeclineReason::LimitExceeded,
        };
        registry.remember_answer(&gold, &item, &answer);
        assert_eq!(registry.recent_answer(&gold, &item), Some(answer));
        assert!(
            registry.recent_answer(&gold, &sku("gems_100")).is_none(),
            "a different item is a different purchase"
        );
        Ok(())
    }

    #[test]
    fn the_paying_card_clears_every_item_and_the_blocked_card_none() -> Result<(), UidError> {
        let registry = Registry::demo()?;
        for item in ["gems_100", "gems_500", "gems_1200"] {
            assert!(
                registry.clear(&uid("8FF14EF1")?, &sku(item)).is_ok(),
                "the Gold card must be approved for {item} on stage"
            );
            assert_eq!(
                registry.clear(&uid("C95DD006")?, &sku(item)).err(),
                Some(DeclineReason::LimitExceeded),
                "the Blocked card must be declined for {item} on stage"
            );
        }
        Ok(())
    }

    #[test]
    fn the_demo_registry_keeps_every_card() -> Result<(), UidError> {
        assert_eq!(Registry::demo()?.card_count(), 5);
        Ok(())
    }
}
