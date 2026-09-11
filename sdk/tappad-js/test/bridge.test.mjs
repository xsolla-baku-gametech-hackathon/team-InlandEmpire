import { test, beforeEach } from "node:test";
import assert from "node:assert/strict";

import { connectBridge, parsePadEvent } from "../index.js";
import { FakeWebSocket } from "./fake-websocket.mjs";

beforeEach(() => {
  FakeWebSocket.instances.length = 0;
});

test("the three pad events parse and the uid is normalised", () => {
  assert.deepEqual(parsePadEvent('{"event":"ready","firmware":"0.1.0"}'), { event: "ready", firmware: "0.1.0" });
  assert.deepEqual(parsePadEvent('{"event":"tap","uid":"04:a3:b2:c1"}'), { event: "tap", uid: "04A3B2C1" });
  assert.deepEqual(parsePadEvent('{"event":"error","message":"reader timeout"}'), {
    event: "error",
    message: "reader timeout",
  });
});

test("anything off-protocol is null, never thrown", () => {
  for (const bad of ["not json", "null", "[]", '{"event":"boot"}', '{"event":"tap"}', '{"event":"tap","uid":"zz"}']) {
    assert.equal(parsePadEvent(bad), null, bad);
  }
});

test("events reach the handler and link state is reported", () => {
  const events = [];
  const links = [];
  const bridge = connectBridge("ws://b", { onEvent: (e) => events.push(e), onLink: (up) => links.push(up) }, {
    WebSocket: FakeWebSocket,
  });
  const [ws] = FakeWebSocket.instances;
  assert.equal(ws.url, "ws://b");
  ws.serverOpens();
  ws.serverSends('{"event":"tap","uid":"04A3B2C1"}');
  ws.serverSends("boot noise");
  bridge.stop();
  assert.deepEqual(events, [{ event: "tap", uid: "04A3B2C1" }]);
  assert.deepEqual(links, [true, false]);
});

test("a dropped connection is reopened after the retry gap", async () => {
  const bridge = connectBridge("ws://b", { onEvent() {} }, { WebSocket: FakeWebSocket, retryMs: 5 });
  FakeWebSocket.instances[0].serverDrops();
  await new Promise((r) => setTimeout(r, 20));
  assert.equal(FakeWebSocket.instances.length, 2, "a second socket was opened");
  bridge.stop();
});

test("stop closes the socket and does not reconnect", async () => {
  const bridge = connectBridge("ws://b", { onEvent() {} }, { WebSocket: FakeWebSocket, retryMs: 5 });
  bridge.stop();
  await new Promise((r) => setTimeout(r, 20));
  assert.equal(FakeWebSocket.instances.length, 1);
  assert.equal(FakeWebSocket.instances[0].closed, true);
});

test("without any WebSocket the error is immediate and says what to do", () => {
  assert.throws(() => connectBridge("ws://b", { onEvent() {} }, { WebSocket: undefined }), /deps.WebSocket/);
});
