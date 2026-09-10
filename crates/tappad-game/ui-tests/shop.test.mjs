// Pins the game flow in docs/protocol.md: buy -> tap -> POST -> approved | declined.
import { test } from "node:test";
import assert from "node:assert/strict";

import { initialState, transition } from "../ui/shop.js";

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

test("buy then tap asks the server exactly once with uid and sku", () => {
  const { state, effects } = play([
    { type: "buy", sku: "gems_500" },
    { type: "tap", uid: "04A3B2C1" },
  ]);
  assert.equal(state.name, "purchasing");
  assert.deepEqual(effects, [{ type: "purchase", uid: "04A3B2C1", sku: "gems_500" }]);
});

test("approved grants the catalogue amount for the sku that was bought", () => {
  const { state } = play([
    { type: "buy", sku: "gems_500" },
    { type: "tap", uid: "04A3B2C1" },
    { type: "response", response: { status: "approved", order_id: 7, receipt_id: "rcpt-000007" } },
  ]);
  assert.equal(state.name, "result");
  assert.equal(state.ok, true);
  assert.equal(state.gems, 500);
  assert.match(state.text, /rcpt-000007/);
});

test("declined keeps gems and shows the reason text, then dismiss returns to browsing", () => {
  const declined = play([
    { type: "buy", sku: "gems_500" },
    { type: "tap", uid: "0B1C2D3E" },
    { type: "response", response: { status: "declined", reason: "limit_exceeded" } },
  ]);
  assert.equal(declined.state.ok, false);
  assert.equal(declined.state.gems, 0);
  assert.equal(declined.state.text, "Over this card's spending limit.");
  const back = play([{ type: "dismiss" }], declined.state);
  assert.equal(back.state.name, "browsing");
  assert.equal(back.state.gems, 0);
});

test("a tap while browsing does not buy anything", () => {
  const { state, effects } = play([{ type: "tap", uid: "04A3B2C1" }]);
  assert.equal(state.name, "browsing");
  assert.deepEqual(effects, []);
});

test("a network failure is a result, not a stuck purchase", () => {
  const { state } = play([
    { type: "buy", sku: "gems_100" },
    { type: "tap", uid: "04A3B2C1" },
    { type: "failure", message: "HTTP 502" },
  ]);
  assert.equal(state.name, "result");
  assert.equal(state.ok, false);
  assert.match(state.text, /502/);
});

test("unknown sku is refused before any server call", () => {
  const { state, effects } = play([{ type: "buy", sku: "gems_999" }]);
  assert.equal(state.name, "browsing");
  assert.deepEqual(effects, []);
});
