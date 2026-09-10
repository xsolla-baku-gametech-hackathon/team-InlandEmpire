import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";

import { createTapPad, isSuccess, DEFAULT_BRIDGE_URL, DEFAULT_SERVER_URL } from "../index.js";
import { fakeFetch } from "./fake-fetch.mjs";
import { FakeWebSocket } from "./fake-websocket.mjs";

beforeEach(() => {
  FakeWebSocket.instances.length = 0;
});

test("defaults point at the demo ports", () => {
  const tappad = createTapPad({}, { WebSocket: FakeWebSocket });
  assert.equal(FakeWebSocket.instances[0].url, DEFAULT_BRIDGE_URL);
  assert.equal(DEFAULT_SERVER_URL, "http://127.0.0.1:8080");
  tappad.stop();
});

test("a tap resolves nextTap and buys through the server", async () => {
  const events = [];
  const fetch = fakeFetch((url) =>
    url.endsWith("/purchase")
      ? { body: { status: "pending_payment", order_id: 3, checkout_url: "https://checkout" } }
      : { body: { order_id: 3, state: "paid" } },
  );
  const tappad = createTapPad(
    { serverUrl: "http://s", bridgeUrl: "ws://b", onEvent: (e) => events.push(e.event) },
    { WebSocket: FakeWebSocket, fetch },
  );
  const tap = tappad.nextTap();
  const [ws] = FakeWebSocket.instances;
  ws.serverOpens();
  ws.serverSends('{"event":"ready","firmware":"0.1.0"}');
  ws.serverSends('{"event":"tap","uid":"04:a3:b2:c1"}');
  const uid = await tap;
  assert.equal(uid, "04A3B2C1");

  const answer = await tappad.buy(uid, "gems_500");
  assert.equal(answer.status, "pending_payment");
  assert.equal(isSuccess(await tappad.waitForPayment(answer.order_id)), true);
  assert.deepEqual(events, ["ready", "tap"], "every event reached onEvent");
  tappad.stop();
});

test("two callers waiting for a tap both get the same card", async () => {
  const tappad = createTapPad({ bridgeUrl: "ws://b" }, { WebSocket: FakeWebSocket });
  const a = tappad.nextTap();
  const b = tappad.nextTap();
  FakeWebSocket.instances[0].serverSends('{"event":"tap","uid":"04A3B2C1"}');
  assert.deepEqual(await Promise.all([a, b]), ["04A3B2C1", "04A3B2C1"]);
  tappad.stop();
});
