# Messages

Every message is one JSON object. Types live in `crates/tappad-protocol`. The
firmware writes the same shapes by hand.

## Pad to bridge, one line per message over USB serial, 115200 baud

```json
{"event":"ready","firmware":"0.1.0"}
{"event":"tap","uid":"04A3B2C1"}
{"event":"error","message":"reader timeout"}
```

`uid` is uppercase hex, no separators. The bridge accepts `04:A3:B2:C1` and
`04 a3 b2 c1` too and normalises.

## Bridge to game, WebSocket text frames on `ws://127.0.0.1:8765`

Same objects as above, forwarded unchanged.

## Game to server

`POST http://127.0.0.1:8080/purchase`

```json
{"uid":"04A3B2C1","sku":"gems_500"}
```

Responses, one of:

```json
{"status":"pending_payment","order_id":12345,"checkout_url":"https://sandbox-secure.xsolla.com/paystation4/?token=..."}
{"status":"approved","order_id":7,"receipt_id":"rcpt-000007"}
{"status":"declined","reason":"limit_exceeded"}
```

`reason` is one of `unknown_card`, `limit_exceeded`, `insufficient_funds`,
`unknown_sku`. A decline is HTTP 200. HTTP 502 means the provider failed.

Any non-2xx answer carries one body shape:

```json
{"error":"provider failed"}
```

`GET http://127.0.0.1:8080/orders/12345`

```json
{"order_id":12345,"state":"paid"}
```

`state` is one of `new`, `paid`, `done`, `canceled`, `expired`.

## Game flow

```
Browsing -> click Buy -> WaitingForTap -> tap -> POST /purchase
  pending_payment -> AwaitingCheckout (iframe open, poll every 800 ms)
      paid or done -> grant gems -> Result
      canceled or expired -> Result "Payment was cancelled"
  approved -> grant gems -> Result
  declined -> Result with the reason text
```
