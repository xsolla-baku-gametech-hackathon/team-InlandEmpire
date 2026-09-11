# tappad-server

HTTP server on `127.0.0.1:8080`. Holds the card registry (which card may spend
how much), and one `PaymentProvider` with two implementations: `xsolla`
(creates a real order, returns the checkout URL, reports paid or not) and
`mock` (approves instantly after the registry check). `TAPPAD_PROVIDER`
picks one. The Xsolla API key is read from `.env` and never leaves this
process.

Routes: `GET /catalog`, `POST /purchase {uid, sku}`, `GET /orders/{id}`.
Full shapes and status codes are in [`docs/protocol.md`](../../docs/protocol.md).

```
cargo run -p tappad-server
```
