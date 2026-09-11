// The one handle a game holds: the pad on one side, the server on the other.

import { connectBridge } from "./bridge.js";
import { createServerClient } from "./server.js";

/** Where a `make demo` laptop has the server and the bridge. */
export const DEFAULT_SERVER_URL = "http://127.0.0.1:8080";
export const DEFAULT_BRIDGE_URL = "ws://127.0.0.1:8765";

/**
 * Connects to the bridge and points at the server.
 *
 *   const tappad = createTapPad();
 *   const uid = await tappad.nextTap();
 *   const answer = await tappad.buy(uid, "gems_500");
 *   if (answer.status === "pending_payment") {
 *     openInGameWindow(answer.checkout_url);
 *     if (isSuccess(await tappad.waitForPayment(answer.order_id))) grant();
 *   }
 *
 * @param {{serverUrl?: string, bridgeUrl?: string, onEvent?: (event: object) => void, onLink?: (up: boolean) => void}} [config]
 *   `onEvent` sees every pad event (ready, tap, error) as it happens;
 *   `onLink` sees the bridge connection go up and down.
 * @param {{fetch?: typeof fetch, WebSocket?: typeof WebSocket, retryMs?: number, pollIntervalMs?: number, pollTimeoutMs?: number, sleep?: (ms: number) => Promise<void>}} [deps]
 *   swapped in by tests; a game leaves them out.
 */
export function createTapPad(config = {}, deps = {}) {
  const server = createServerClient(config.serverUrl ?? DEFAULT_SERVER_URL, deps);
  const waiting = [];
  const bridge = connectBridge(
    config.bridgeUrl ?? DEFAULT_BRIDGE_URL,
    {
      onEvent(event) {
        config.onEvent?.(event);
        if (event.event === "tap") {
          for (const resolve of waiting.splice(0)) resolve(event.uid);
        }
      },
      onLink: config.onLink,
    },
    deps,
  );

  return {
    /** Resolves with the next card held on the pad. Waits as long as it takes. */
    nextTap() {
      return new Promise((resolve) => waiting.push(resolve));
    },
    /** GET /catalog. */
    catalog: () => server.catalog(),
    /** POST /purchase. A decline resolves, it does not reject. */
    buy: (uid, sku) => server.purchase(uid, sku),
    /** Polls GET /orders/{id} until the state is final and resolves with it. */
    waitForPayment: (orderId) => server.waitUntilFinal(orderId),
    /** Closes the bridge connection. Pending nextTap() promises never settle. */
    stop: () => bridge.stop(),
    /** The HTTP side alone, for a game that gets taps some other way. */
    server,
  };
}
