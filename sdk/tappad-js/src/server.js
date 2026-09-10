// HTTP client for tappad-server. Three routes, shapes in docs/protocol.md.

import { TapPadError } from "./error.js";

const PURCHASE_STATUSES = new Set(["pending_payment", "approved", "declined"]);

/**
 * @param {string} baseUrl  "http://127.0.0.1:8080"
 * @param {{fetch?: typeof fetch}} [deps]  a fetch to use instead of the global one
 */
export function createServerClient(baseUrl, deps = {}) {
  const base = baseUrl.replace(/\/+$/, "");
  const doFetch = deps.fetch ?? globalThis.fetch;

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
  };
}
