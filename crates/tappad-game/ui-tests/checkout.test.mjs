// Pins the pending_payment branch of docs/protocol.md: iframe open, poll, grant on paid or done.
import { test } from "node:test";
import assert from "node:assert/strict";

import { initialState, transition } from "../ui/shop.js";

const pending = {
  status: "pending_payment",
  order_id: 12345,
  checkout_url: "https://sandbox-secure.xsolla.com/paystation4/?token=abc",
};

function play(events, from = initialState) {
  let state = from;
  const effects = [];
  for (const event of events) {
    const out = transition(state, event);
    state = out.state;
    effects.push(...out.effects);
  }
  return { state, effects };
}

function toCheckout() {
  return play([
    { type: "buy", sku: "gems_500" },
    { type: "tap", uid: "04A3B2C1" },
    { type: "response", response: pending },
  ]);
}

test("pending_payment opens the checkout url and starts polling that order", () => {
  const { state, effects } = toCheckout();
  assert.equal(state.name, "awaiting_checkout");
  assert.equal(state.orderId, 12345);
  assert.deepEqual(effects.slice(1), [
    { type: "open_checkout", url: pending.checkout_url },
    { type: "poll", orderId: 12345 },
  ]);
});

test("new keeps waiting; paid grants gems, closes checkout and stops polling", () => {
  const { state: waiting } = toCheckout();
  const still = play([{ type: "order", state: "new" }], waiting);
  assert.equal(still.state.name, "awaiting_checkout");
  assert.deepEqual(still.effects, []);

  const paid = play([{ type: "order", state: "paid" }], waiting);
  assert.equal(paid.state.name, "result");
  assert.equal(paid.state.ok, true);
  assert.equal(paid.state.gems, 500);
  assert.deepEqual(paid.effects, [{ type: "stop_polling" }, { type: "close_checkout" }]);
});

test("done also grants; canceled and expired do not", () => {
  const { state: waiting } = toCheckout();
  assert.equal(play([{ type: "order", state: "done" }], waiting).state.gems, 500);
  for (const s of ["canceled", "expired"]) {
    const out = play([{ type: "order", state: s }], waiting);
    assert.equal(out.state.gems, 0, s);
    assert.equal(out.state.text, "Payment was cancelled.", s);
  }
});

test("one failed poll does not end the purchase", () => {
  const { state: waiting } = toCheckout();
  const out = play([{ type: "failure", message: "HTTP 502" }], waiting);
  assert.equal(out.state.name, "awaiting_checkout");
  assert.deepEqual(out.effects, []);
});

test("cancelling from the game closes checkout and keeps gems", () => {
  const { state: waiting } = toCheckout();
  const out = play([{ type: "dismiss" }], waiting);
  assert.equal(out.state.name, "result");
  assert.equal(out.state.gems, 0);
  assert.deepEqual(out.effects, [{ type: "stop_polling" }, { type: "close_checkout" }]);
});
