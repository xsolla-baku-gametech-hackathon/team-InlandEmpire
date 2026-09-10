// Run with: node --test crates/tappad-game/ui-tests
// Pins the pad event shapes from docs/protocol.md on the game side.
import { test } from "node:test";
import assert from "node:assert/strict";

import { parsePadEvent, normaliseUid } from "../ui/bridge.js";

test("tap uid is normalised like the bridge and server do it", () => {
  assert.equal(normaliseUid("04:a3:b2:c1"), "04A3B2C1");
  assert.equal(normaliseUid("04 a3 b2 c1"), "04A3B2C1");
  assert.deepEqual(parsePadEvent('{"event":"tap","uid":"0B:1C:2D:3E"}'), {
    event: "tap",
    uid: "0B1C2D3E",
  });
});

test("ready and error pass through as the doc shows them", () => {
  assert.deepEqual(parsePadEvent('{"event":"ready","firmware":"0.1.0"}'), {
    event: "ready",
    firmware: "0.1.0",
  });
  assert.deepEqual(parsePadEvent('{"event":"error","message":"reader timeout"}'), {
    event: "error",
    message: "reader timeout",
  });
});

test("anything off-protocol is dropped, never thrown", () => {
  for (const bad of ['{"event":"boot"}', "not json", '{"event":"tap"}', "null", "[]"]) {
    assert.equal(parsePadEvent(bad), null, bad);
  }
});
