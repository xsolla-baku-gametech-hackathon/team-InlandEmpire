// The shop state machine. Pure: (state, event) -> { state, effects }.
// Every transition lives here so the flow in docs/protocol.md is readable in one file.

/** Gems granted per SKU. Must match the server catalogue. */
export const CATALOGUE = Object.freeze({
  gems_100: 100,
  gems_500: 500,
  gems_1200: 1200,
});

/** Player-facing text per decline reason from docs/protocol.md. */
export const DECLINE_TEXT = Object.freeze({
  unknown_card: "This card is not registered.",
  limit_exceeded: "Over this card's spending limit.",
  insufficient_funds: "The payment was refused.",
  unknown_sku: "That item is not for sale.",
});

export const initialState = Object.freeze({ name: "browsing", gems: 0, note: "Pick an item." });

/**
 * @param {object} state  current state
 * @param {object} event  one of: buy, tap, response, failure, dismiss
 * @returns {{ state: object, effects: object[] }}
 */
export function transition(state, event) {
  switch (state.name) {
    case "browsing":
      return browsing(state, event);
    case "waiting_for_tap":
      return waitingForTap(state, event);
    case "purchasing":
      return purchasing(state, event);
    case "result":
      return result(state, event);
    default:
      return stay(state);
  }
}

function browsing(state, event) {
  switch (event.type) {
    case "buy":
      if (!(event.sku in CATALOGUE)) return stay(state);
      return next({ name: "waiting_for_tap", gems: state.gems, sku: event.sku });
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
        { name: "purchasing", gems: state.gems, sku: state.sku, uid: event.uid },
        [{ type: "purchase", uid: event.uid, sku: state.sku }],
      );
    case "dismiss":
      return next({ name: "browsing", gems: state.gems, note: "Pick an item." });
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
      const granted = CATALOGUE[state.sku];
      return next({
        name: "result",
        gems: state.gems + granted,
        ok: true,
        text: `Paid. Added ${granted} gems. Receipt ${response.receipt_id}.`,
      });
    }
    case "declined":
      return next({
        name: "result",
        gems: state.gems,
        ok: false,
        text: DECLINE_TEXT[response.reason] ?? `Declined: ${response.reason}.`,
      });
    case "pending_payment":
      // Checkout iframe and polling arrive with the next task; until then say so honestly.
      return next({
        name: "result",
        gems: state.gems,
        ok: false,
        text: `Order ${response.order_id} needs checkout, which this build does not open yet.`,
      });
    default:
      return next({
        name: "result",
        gems: state.gems,
        ok: false,
        text: "The server sent an unknown answer.",
      });
  }
}

function result(state, event) {
  if (event.type === "dismiss") {
    return next({ name: "browsing", gems: state.gems, note: "Pick an item." });
  }
  return stay(state);
}

function next(state, effects = []) {
  return { state, effects };
}

function stay(state) {
  return { state, effects: [] };
}
