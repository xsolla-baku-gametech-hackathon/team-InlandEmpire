# tappad-sdk

The crate a game depends on to sell through TapPad. It hides the WebSocket to
the bridge and the HTTP calls to the server behind one handle, `TapPad`, and
re-exports the wire types from `tappad-protocol` so the game needs one
dependency.

```toml
[dependencies]
tappad-sdk = { path = "../tappad-sdk" }   # or the git URL of this repo
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## Set up the payment system

The SDK talks to two local processes. Both come from this repo.

1. **The server** holds the card registry and the Xsolla key.

   ```
   cp .env.example .env
   cargo run -p tappad-server
   ```

   With `TAPPAD_PROVIDER=mock` (the default) every purchase is approved on the
   spot and nothing leaves the laptop. For real sandbox orders set
   `TAPPAD_PROVIDER=xsolla`, `XSOLLA_PROJECT_ID` and `XSOLLA_API_KEY`; see
   `docs/xsolla.md` for where those come from. The key is read by the server
   only. The SDK never sees it and never needs it.

2. **The bridge** turns the USB pad into WebSocket frames.

   ```
   cargo run -p tappad-bridge -- --fake        # a tap every 5 s, no hardware
   cargo run -p tappad-bridge -- --port COM3   # the real pad
   ```

3. **Cards** are an allowlist in `crates/tappad-server/src/registry.rs`
   (`Registry::demo`). Each card has a per-tap spending limit in cents. Read a
   new card's UID off the bridge log and add it there.

4. **Items** come from the Xsolla Store catalogue, or a fixed list of three gem
   packs with the mock provider. `GET /catalog` returns whatever the server has,
   and `TapPad::catalog` gives it to you typed.

`Config::default()` points at the demo ports, `127.0.0.1:8080` and
`127.0.0.1:8765`. Change `server_url` and `bridge_url` if you move them.

## Sell something

```rust
use tappad_sdk::{Config, PurchaseResponse, Sku, TapPad};

let mut tappad = TapPad::connect(&Config::default())?;
let uid = tappad.next_tap().await;                       // blocks until a card
match tappad.buy(uid, Sku::new("gems_500")).await? {
    PurchaseResponse::Approved { .. } => grant(),
    PurchaseResponse::Declined { reason } => show(reason.message()),
    PurchaseResponse::PendingPayment { order_id, checkout_url } => {
        open_in_game_window(&checkout_url);
        if tappad.wait_for_payment(order_id).await?.is_success() { grant() }
    }
}
```

The full version, with the catalogue printed and every branch handled, is
`examples/tap_to_gems.rs`:

```
cargo run -p tappad-sdk --example tap_to_gems
```

## What each call does

| Call | Wire | Notes |
|---|---|---|
| `TapPad::next_tap` | bridge WebSocket | Reconnects for as long as you wait. Never fails. |
| `TapPad::catalog` | `GET /catalog` | Items in store order, price in cents. |
| `TapPad::buy` | `POST /purchase` | A decline is `Ok(Declined)`, not an error. |
| `TapPad::wait_for_payment` | `GET /orders/{id}` every 800 ms | Stops at a final state or after five minutes. |

`TapPad::pad()` gives the raw `Pad` for `ready` and `error` events;
`TapPad::server()` gives the `ServerClient` when taps come from somewhere else.

## Errors

One enum, `SdkError`. `Transport` means the server is down. `Server` carries
the HTTP status and the `error` text the server sent, for example 502 when
Xsolla failed or 404 for an order it never made. `Protocol` means the body
was not the documented shape. `PollTimeout` names the order that never
settled. Business answers such as "limit exceeded" are never errors; they are
a `PurchaseResponse::Declined` with a `DeclineReason` that has player-facing
text in `reason.message()`.

## Not in this crate

No UI: the game's page, Tauri window and gem counter live in `tappad-game`,
which does not use this crate. No Xsolla calls: those stay in the server so
the API key does. No card enrolment: the registry is in code.
