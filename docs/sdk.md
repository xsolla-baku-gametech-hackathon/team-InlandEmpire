# SDKs for games

Two client libraries, one per language, so a game never writes HTTP or
WebSocket code against `docs/protocol.md` by hand. Both are bonus material:
the demo game in `crates/tappad-game` predates them and does not use either.

| | Rust | JavaScript |
|---|---|---|
| Where | `crates/tappad-sdk` | `sdk/tappad-js` |
| Import | `tappad-sdk` crate | `sdk/tappad-js/index.js`, ES module, no deps |
| Handle | `TapPad::connect(&Config)` | `createTapPad(config)` |
| Next card | `next_tap().await` | `await nextTap()` |
| Buy | `buy(uid, sku)` | `buy(uid, sku)` |
| Wait for checkout | `wait_for_payment(order_id)` | `waitForPayment(orderId)` |
| Both at once | `buy_and_settle(uid, sku, open_checkout)` | not yet |
| Errors | `SdkError` enum | `TapPadError` with `kind` |
| Tests | `cargo test -p tappad-sdk` | `node --test "sdk/tappad-js/test/*.test.mjs"` |

Both follow the same rules as the rest of the repo: a decline is an answer,
not an error; money is integer cents; order states are named, never compared
as strings; the Xsolla key stays in the server. Setup for the server, bridge,
cards and items is the same for both and is written once in each README.

The Rust tests start a real `tappad-server` with the mock provider and a real
bridge on loopback ports. The JavaScript tests use a scripted `fetch` and a
fake `WebSocket`. Neither reaches the network.
