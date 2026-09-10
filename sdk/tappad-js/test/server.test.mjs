import { test } from "node:test";
import assert from "node:assert/strict";

import { createServerClient, isFinal, isSuccess, TapPadError } from "../index.js";
import { fakeFetch } from "./fake-fetch.mjs";

const gems = [
  { sku: "gems_100", name: "100 gems", description: "100 gems", price: 99, currency: "USD", image_url: null },
];

test("catalog returns the list the server sent", async () => {
  const fetch = fakeFetch(() => ({ body: gems }));
  const server = createServerClient("http://s/", { fetch });
  assert.deepEqual(await server.catalog(), gems);
  assert.equal(fetch.calls[0].url, "http://s/catalog", "trailing slash is trimmed");
});

test("a dead server is a transport error", async () => {
  const server = createServerClient("http://s", { fetch: fakeFetch(() => new Error("ECONNREFUSED")) });
  await assert.rejects(server.catalog(), (err) => err instanceof TapPadError && err.kind === "transport");
});

test("a non-2xx answer carries its status and the error text", async () => {
  const server = createServerClient("http://s", {
    fetch: fakeFetch(() => ({ status: 502, body: { error: "provider failed" } })),
  });
  await assert.rejects(server.catalog(), (err) => {
    assert.equal(err.kind, "server");
    assert.equal(err.status, 502);
    assert.match(err.message, /provider failed/);
    return true;
  });
});

test("a non-2xx answer without a body falls back to the status text", async () => {
  const server = createServerClient("http://s", { fetch: fakeFetch(() => ({ status: 404 })) });
  await assert.rejects(server.catalog(), (err) => err.kind === "server" && /Not Found/.test(err.message));
});

test("a 2xx body that is not a catalogue is a protocol error", async () => {
  const server = createServerClient("http://s", { fetch: fakeFetch(() => ({ body: { nope: 1 } })) });
  await assert.rejects(server.catalog(), (err) => err.kind === "protocol");
});

test("purchase posts uid and sku as the protocol shows them", async () => {
  const fetch = fakeFetch(() => ({ body: { status: "approved", order_id: 7, receipt_id: "rcpt-000007" } }));
  const server = createServerClient("http://s", { fetch });
  const answer = await server.purchase("04A3B2C1", "gems_500");
  assert.equal(answer.status, "approved");
  assert.equal(answer.order_id, 7);
  const { url, init } = fetch.calls[0];
  assert.equal(url, "http://s/purchase");
  assert.equal(init.method, "POST");
  assert.deepEqual(JSON.parse(init.body), { uid: "04A3B2C1", sku: "gems_500" });
});

test("a decline is an answer, not a rejection", async () => {
  const server = createServerClient("http://s", {
    fetch: fakeFetch(() => ({ body: { status: "declined", reason: "limit_exceeded" } })),
  });
  assert.deepEqual(await server.purchase("04A3B2C1", "gems_500"), {
    status: "declined",
    reason: "limit_exceeded",
  });
});

test("a purchase answer with an unknown status is a protocol error", async () => {
  const server = createServerClient("http://s", { fetch: fakeFetch(() => ({ body: { status: "maybe" } })) });
  await assert.rejects(server.purchase("04A3B2C1", "gems_500"), (err) => err.kind === "protocol");
});

test("order states: every final state is named, only paid and done grant", () => {
  for (const s of ["paid", "done", "canceled", "expired"]) assert.equal(isFinal(s), true, s);
  assert.equal(isFinal("new"), false);
  assert.equal(isSuccess("paid"), true);
  assert.equal(isSuccess("done"), true);
  assert.equal(isSuccess("canceled"), false);
  assert.equal(isSuccess("new"), false);
});

test("waitUntilFinal polls until the order is final", async () => {
  const states = ["new", "new", "paid"];
  const fetch = fakeFetch(() => ({ body: { order_id: 5, state: states.shift() } }));
  const slept = [];
  const server = createServerClient("http://s", {
    fetch,
    pollIntervalMs: 800,
    sleep: async (ms) => void slept.push(ms),
  });
  assert.equal(await server.waitUntilFinal(5), "paid");
  assert.equal(fetch.calls.length, 3);
  assert.deepEqual(slept, [800, 800]);
  assert.equal(fetch.calls[0].url, "http://s/orders/5");
});

test("waitUntilFinal gives up at the deadline and names the order", async () => {
  const server = createServerClient("http://s", {
    fetch: fakeFetch(() => ({ body: { order_id: 9, state: "new" } })),
    pollIntervalMs: 5,
    pollTimeoutMs: 20,
    sleep: (ms) => new Promise((r) => setTimeout(r, ms)),
  });
  await assert.rejects(server.waitUntilFinal(9), (err) => err.kind === "poll_timeout" && err.orderId === 9);
});

test("an unknown order surfaces the server's 404", async () => {
  const server = createServerClient("http://s", {
    fetch: fakeFetch(() => ({ status: 404, body: { error: "unknown order" } })),
  });
  await assert.rejects(server.orderStatus(1), (err) => err.kind === "server" && err.status === 404);
});
