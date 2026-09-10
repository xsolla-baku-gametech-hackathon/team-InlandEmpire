import { test } from "node:test";
import assert from "node:assert/strict";

import { createServerClient, TapPadError } from "../index.js";
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
