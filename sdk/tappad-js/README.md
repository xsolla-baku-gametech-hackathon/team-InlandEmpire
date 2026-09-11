# tappad-sdk for JavaScript

The package a browser or Tauri game imports to sell through TapPad. Plain ES
modules, no dependencies, no build step. It needs `fetch` and `WebSocket`,
which every browser, Tauri webview and Node 22 or newer has.

```js
import { createTapPad, isSuccess } from "../sdk/tappad-js/index.js";
```

Copy the folder, or point at it in the repo. It is not published to npm.

## Set up the payment system

The SDK talks to two local processes from this repo.

1. **The server** holds the card registry and the Xsolla key.

   ```
   cp .env.example .env
   cargo run -p tappad-server
   ```

   `TAPPAD_PROVIDER=mock` (the default) approves every purchase on the spot,
   offline. For real sandbox orders set `TAPPAD_PROVIDER=xsolla`,
   `XSOLLA_PROJECT_ID` and `XSOLLA_API_KEY`; `docs/xsolla.md` says where they
   come from. The key never reaches the game or this SDK.

2. **The bridge** turns the USB pad into WebSocket frames.

   ```
   cargo run -p tappad-bridge -- --fake        # a tap every 5 s, no hardware
   cargo run -p tappad-bridge -- --port COM3   # the real pad
   ```

3. **Cards** are an allowlist in `crates/tappad-server/src/registry.rs`
   (`Registry::demo`), each with a per-tap spending limit in cents. Read a new
   card's UID off the bridge log and add it there.

4. **Items** come from the Xsolla Store catalogue, or three fixed gem packs
   with the mock provider. `tappad.catalog()` returns them with `price` in
   cents.

Defaults are `http://127.0.0.1:8080` and `ws://127.0.0.1:8765`. Pass
`serverUrl` and `bridgeUrl` to move them.

## Sell something

```js
const tappad = createTapPad({
  onLink: (up) => showPadStatus(up ? "pad connected" : "pad offline"),
});

const uid = await tappad.nextTap();                 // resolves on the next card
const answer = await tappad.buy(uid, "gems_500");
switch (answer.status) {
  case "approved":
    grant();
    break;
  case "declined":
    show(answer.reason);                            // "limit_exceeded", ...
    break;
  case "pending_payment":
    openInGameWindow(answer.checkout_url);          // an iframe works
    if (isSuccess(await tappad.waitForPayment(answer.order_id))) grant();
    break;
}
```

## What each call does

| Call | Wire | Notes |
|---|---|---|
| `nextTap()` | bridge WebSocket | Reconnects for as long as you wait. Never rejects. |
| `catalog()` | `GET /catalog` | Items in store order, price in cents. |
| `buy(uid, sku)` | `POST /purchase` | A decline resolves with `status: "declined"`. |
| `waitForPayment(orderId)` | `GET /orders/{id}` every 800 ms | Resolves at a final state, rejects after five minutes. |
| `stop()` | | Closes the bridge connection. |

`isFinal(state)` and `isSuccess(state)` name the order states so the game
never compares strings. `parseUid` and `parsePadEvent` are exported for a game
that reads the bridge itself. `tappad.server` is the HTTP client alone.

## Errors

Every rejection is a `TapPadError` with a `kind`: `transport` when the server
is down, `server` with `status` and the server's `error` text (502 when Xsolla
failed, 404 for an order it never made), `protocol` when a 2xx body is not the
documented shape, `poll_timeout` with `orderId` when an order never settled.
A decline is never an error.

## Tests

```
node --test "sdk/tappad-js/test/*.test.mjs"
```

They run against a scripted `fetch` and a fake `WebSocket`. Nothing touches
the network.

## Not in this package

No UI. The demo game in `crates/tappad-game` has its own copies of these
calls and does not import this package. No Xsolla calls: those stay in the
server so the API key does.
