// The shop state machine. Pure: (state, event) -> { state, effects }.
// Every transition lives here so the flow in docs/protocol.md is readable in one file.

/**
 * Items shown until `GET /catalog` answers, and what the tests run against.
 * Same three packs as the mock provider. Prices in cents.
 */
export const DEFAULT_ITEMS = Object.freeze([
  { sku: "gems_100", name: "100 gems", description: "100 gems", price: 99, currency: "USD", image_url: null },
  { sku: "gems_500", name: "500 gems", description: "500 gems", price: 499, currency: "USD", image_url: null },
  { sku: "gems_1200", name: "1200 gems", description: "1200 gems", price: 999, currency: "USD", image_url: null },
]);

/** Gems a catalogue item grants: the number in its name, `"500 gems"` -> 500. */
export function gemsFor(item) {
  const m = /(\d+)/.exec(item?.name ?? "");
  return m ? Number(m[1]) : 0;
}

function itemFor(state, sku) {
  return state.items.find((i) => i.sku === sku);
}

/** How often the game asks `GET /orders/{id}` while the checkout is open. */
export const POLL_MS = 800;

/** Player-facing text per decline reason from docs/protocol.md. */
export const DECLINE_TEXT = Object.freeze({
  unknown_card: "Card declined.",
  limit_exceeded: "Card declined.",
  insufficient_funds: "The payment was refused.",
  unknown_sku: "That item is not for sale.",
});

export const initialState = Object.freeze({
  name: "browsing",
  gems: 0,
  note: "Pick an item.",
  items: DEFAULT_ITEMS,
});

/**
 * @param {object} state  current state
 * @param {object} event  one of: catalog, buy, tap, response, order, failure, dismiss
 * @returns {{ state: object, effects: object[] }}
 */
export function transition(state, event) {
  if (event.type === "catalog") {
    const items = Array.isArray(event.items) && event.items.length > 0 ? event.items : state.items;
    return next({ ...state, items });
  }
  switch (state.name) {
    case "browsing":
      return browsing(state, event);
    case "waiting_for_tap":
      return waitingForTap(state, event);
    case "purchasing":
      return purchasing(state, event);
    case "awaiting_checkout":
      return awaitingCheckout(state, event);
    case "result":
      return result(state, event);
    default:
      return stay(state);
  }
}

function browsing(state, event) {
  switch (event.type) {
    case "buy":
      if (!itemFor(state, event.sku)) return stay(state);
      return next({ name: "waiting_for_tap", gems: state.gems, items: state.items, sku: event.sku });
    case "tap":
      return next({ ...state, note: `Card ${event.uid} tapped. Pick an item first.` });
    default:
      return stay(state);
  }
}

function waitingForTap(state, event) {
  switch (event.type) {
    case "tap":
      return next(
        { name: "purchasing", gems: state.gems, items: state.items, sku: state.sku, uid: event.uid },
        [{ type: "purchase", uid: event.uid, sku: state.sku }],
      );
    case "dismiss":
      return next({ name: "browsing", gems: state.gems, items: state.items, note: "Pick an item." });
    default:
      return stay(state);
  }
}

function purchasing(state, event) {
  switch (event.type) {
    case "response":
      return onResponse(state, event.response);
    case "failure":
      return next({
        name: "result",
        gems: state.gems,
        items: state.items,
        ok: false,
        text: `Could not reach the server: ${event.message}`,
      });
    default:
      return stay(state);
  }
}

function onResponse(state, response) {
  switch (response.status) {
    case "approved": {
      const granted = gemsFor(itemFor(state, state.sku));
      return next({
        name: "result",
        gems: state.gems + granted,
        items: state.items,
        ok: true,
        text: `Paid. Added ${granted} gems. Receipt ${response.receipt_id}.`,
      });
    }
    case "declined":
      return next({
        name: "result",
        gems: state.gems,
        items: state.items,
        ok: false,
        text: DECLINE_TEXT[response.reason] ?? `Declined: ${response.reason}.`,
      });
    case "pending_payment":
      return next(
        {
          name: "awaiting_checkout",
          gems: state.gems,
          items: state.items,
          sku: state.sku,
          orderId: response.order_id,
          checkoutUrl: response.checkout_url,
        },
        [
          { type: "open_checkout", url: response.checkout_url },
          { type: "poll", orderId: response.order_id },
        ],
      );
    default:
      return next({
        name: "result",
        gems: state.gems,
        items: state.items,
        ok: false,
        text: "The server sent an unknown answer.",
      });
  }
}

function awaitingCheckout(state, event) {
  switch (event.type) {
    case "order":
      return onOrderState(state, event.state);
    case "dismiss":
      return next(
        { name: "result", gems: state.gems, items: state.items, ok: false, text: "Payment was cancelled." },
        [{ type: "stop_polling" }, { type: "close_checkout" }],
      );
    case "failure":
      // One failed poll is not a failed payment; keep polling until a final state.
      return stay(state);
    default:
      return stay(state);
  }
}

function onOrderState(state, orderState) {
  const done = [{ type: "stop_polling" }, { type: "close_checkout" }];
  switch (orderState) {
    case "paid":
    case "done": {
      const granted = gemsFor(itemFor(state, state.sku));
      return next(
        {
          name: "result",
          gems: state.gems + granted,
          items: state.items,
          ok: true,
          text: `Paid. Added ${granted} gems. Order ${state.orderId}.`,
        },
        done,
      );
    }
    case "canceled":
    case "expired":
      return next(
        { name: "result", gems: state.gems, items: state.items, ok: false, text: "Payment was cancelled." },
        done,
      );
    case "new":
      return stay(state);
    default:
      return next(
        { name: "result", gems: state.gems, items: state.items, ok: false, text: `Unknown order state: ${orderState}.` },
        done,
      );
  }
}

function result(state, event) {
  if (event.type === "dismiss") {
    return next({ name: "browsing", gems: state.gems, items: state.items, note: "Pick an item." });
  }
  return stay(state);
}

function next(state, effects = []) {
  return { state, effects };
}

function stay(state) {
  return { state, effects: [] };
}
