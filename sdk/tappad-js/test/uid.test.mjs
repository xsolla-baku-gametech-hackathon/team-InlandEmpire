import { test } from "node:test";
import assert from "node:assert/strict";

import { parseUid } from "../index.js";

test("separators go and hex is uppercased", () => {
  assert.equal(parseUid("04:a3:b2:c1"), "04A3B2C1");
  assert.equal(parseUid("04 a3 b2 c1"), "04A3B2C1");
  assert.equal(parseUid("04-a3-b2-c1"), "04A3B2C1");
});

test("only 4, 7 and 10 byte uids exist", () => {
  assert.equal(parseUid("04A3B2C1A1B2C3"), "04A3B2C1A1B2C3");
  assert.equal(parseUid("04A3B2C1A1B2C3D4E5F6"), "04A3B2C1A1B2C3D4E5F6");
  assert.equal(parseUid("04A3B2"), null);
  assert.equal(parseUid(""), null);
});

test("anything that is not hex is rejected", () => {
  assert.equal(parseUid("04A3B2CZ"), null);
  assert.equal(parseUid(42), null);
  assert.equal(parseUid(null), null);
});
