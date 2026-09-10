// HTTP client for tappad-server. Three routes, shapes in docs/protocol.md.

import { TapPadError } from "./error.js";

const PURCHASE_STATUSES = new Set(["pending_payment", "approved", "declined"]);
const FINAL_STATES = new Set(["paid", "done", "canceled", "expired"]);
const SUCCESS_STATES = new Set(["paid", "done"]);

/** True when the order will not change again, so polling can stop. */
export function isFinal(state) {
  return FINAL_STATES.has(state);
}

/** True when the player should get the item. */
export function isSuccess(state) {
  return SUCCESS_STATES.has(state);
}

/**
 * @param {string} baseUrl  "http://127.0.0.1:8080"
 * @param {{fetch?: typeof fetch, pollIntervalMs?: number, pollTimeoutMs?: number, sleep?: (ms: number) => Promise<void>}} [deps]
 *   `fetch` and `sleep` can be swapped for tests; the poll timings default to
 *   800 ms between polls and five minutes overall.
 */
export function createServerClient(baseUrl, deps = {}) {
  const base = baseUrl.replace(/\/+$/, "");
  const doFetch = deps.fetch ?? globalThis.fetch;
  const sleep = deps.sleep ?? ((ms) => new Promise((r) => setTimeout(r, ms)));
  const pollIntervalMs = deps.pollIntervalMs ?? 800;
  const pollTimeoutMs = deps.pollTimeoutMs ?? 5 * 60 * 1000;

  /** Turns one fetch into the typed body or a TapPadError. */
  async function call(path, init) {
    let res;
    try {
      res = await doFetch(`${base}${path}`, init);
    } catch (cause) {
      throw new TapPadError("transport", `server unreachable: ${cause?.message ?? cause}`, { cause });
    }
    if (!res.ok) {
      let message = res.statusText || "no error body";
      try {
        const body = await res.json();
        if (body && typeof body.error === "string") message = body.error;
      } catch {
        // keep the status text
      }
      throw new TapPadError("server", `server answered ${res.status}: ${message}`, {
        status: res.status,
      });
    }
    let body;
    try {
      body = await res.json();
    } catch (cause) {
      throw new TapPadError("protocol", "server answer is not JSON", { cause });
    }
    return body;
  }

  return {
    /** GET /catalog: what the shop can sell, in store order. */
    async catalog() {
      const items = await call("/catalog");
      if (!Array.isArray(items)) throw new TapPadError("protocol", "catalog is not a list");
      return items;
    },

    /**
     * POST /purchase: buy `sku` with the card that was just tapped.
     * A decline resolves to `{status: "declined", reason}`; it is an answer,
     * not an error. Resolves to one of the three PurchaseResponse shapes.
     * @param {string} uid  normalised card uid
     * @param {string} sku
     */
    async purchase(uid, sku) {
      const answer = await call("/purchase", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ uid, sku }),
      });
      if (!answer || !PURCHASE_STATUSES.has(answer.status)) {
        throw new TapPadError("protocol", `unknown purchase status ${JSON.stringify(answer?.status)}`);
      }
      return answer;
    },

    /**
     * GET /orders/{id}: where the order is right now.
     * @param {number} orderId
     * @returns {Promise<{order_id: number, state: string}>}
     */
    async orderStatus(orderId) {
      const status = await call(`/orders/${orderId}`);
      if (!status || typeof status.state !== "string") {
        throw new TapPadError("protocol", "order status has no state");
      }
      return status;
    },

    /**
     * Polls the order until it is final and resolves to that state. Call it
     * after a `pending_payment` answer while the checkout is open. Rejects with
     * kind "poll_timeout" when the deadline passes first.
     * @param {number} orderId
     * @returns {Promise<string>}  "paid" | "done" | "canceled" | "expired"
     */
    async waitUntilFinal(orderId) {
      const deadline = Date.now() + pollTimeoutMs;
      for (;;) {
        const { state } = await this.orderStatus(orderId);
        if (isFinal(state)) return state;
        if (Date.now() + pollIntervalMs > deadline) {
          throw new TapPadError("poll_timeout", `order ${orderId} still not final after the poll timeout`, {
            orderId,
          });
        }
        await sleep(pollIntervalMs);
      }
    },
  };
}
