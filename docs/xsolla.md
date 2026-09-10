# Xsolla, the two calls we make

Both verified against developers.xsolla.com on Sept 9, 2026.

## 1. Create a payment token and order (server only)

```
POST https://store.xsolla.com/api/v3/project/{project_id}/admin/payment/token
Authorization: Basic base64(project_id:api_key)
```

Body:

```json
{
  "sandbox": true,
  "user": { "id": { "value": "dad" }, "country": { "value": "US", "allow_modify": false } },
  "purchase": { "items": [ { "sku": "gems_500", "quantity": 1 } ] },
  "settings": { "ui": { "layout": "embed", "theme": "63295aab2e47fab76f7708e3" } }
}
```

Response `201`:

```json
{ "token": "huooAqbXBSJxB8Q4dYBqJp4ybiInqsPb", "order_id": 12345 }
```

`401` means the key or project id is wrong. `422` means the body is wrong,
usually a missing country or an SKU that is not in the catalogue.

Try it from a shell before writing Rust:

```
curl -s -X POST \
  -u "$XSOLLA_PROJECT_ID:$XSOLLA_API_KEY" \
  "https://store.xsolla.com/api/v3/project/$XSOLLA_PROJECT_ID/admin/payment/token" \
  -H 'Content-Type: application/json' \
  -d '{"sandbox":true,"user":{"id":{"value":"dad"},"country":{"value":"US","allow_modify":false}},"purchase":{"items":[{"sku":"gems_500","quantity":1}]},"settings":{"ui":{"layout":"embed","theme":"63295aab2e47fab76f7708e3"}}}'
```

## 2. Checkout page

```
https://sandbox-secure.xsolla.com/paystation4/?token={token}
```

Production is `https://secure.xsolla.com/paystation4/?token={token}`. The game
loads the sandbox URL in an iframe. Test cards are in the Xsolla docs under
"Test cards", copy them into the deck notes.

## 3. Order status (server, polled by the game through `GET /orders/{id}`)

```
GET https://store.xsolla.com/api/v2/project/{project_id}/order/{order_id}
Authorization: Bearer {token from step 1}
```

The docs say a token for opening the payment UI is accepted here, so no user
login token is needed. Response has `status`. Values we handle: `new`, `paid`,
`done`, `canceled`. Treat `paid` and `done` as success. Confirm the exact
strings against the first real response and fix `OrderState` if they differ.

## Setup in Publisher Account (owner A, before kickoff)

Done on 2026-09-10. Merchant id 937757, project id 315338 (the number after
`/projects/` in the URL, not the one after the host). The API key lives in
`.env` only.

Items were created through the Store admin API because the Publisher Account
item form is a maze. Auth for that call is `merchant_id:api_key`, not
`project_id:api_key`:

```sh
curl -u "937757:$XSOLLA_API_KEY" -H 'content-type: application/json' \
  https://store.xsolla.com/api/v2/project/315338/admin/items/virtual_items \
  -d '{"sku":"gems_500","name":{"en":"500 gems"},"description":{"en":"500 gems"},
       "is_enabled":true,"is_free":false,"is_show_in_store":true,
       "prices":[{"currency":"USD","amount":4.99,"is_default":true,"is_enabled":true}]}'
```

Catalog: `gems_100` 0.99, `gems_500` 4.99, `gems_1200` 9.99 USD. Server-side
prices in `registry.rs` must match. A missing SKU makes the token call answer
`422 Cart is empty`. Paid order 725985390 in the sandbox with Visa `4111 1111 1111 1111`,
expiry `12/40` (any other expiry gives "Error 1055"), any CVV, ZIP 12345.
`GET /orders/725985390` on the server went from `new` to `done` right after
Pay Station showed "Payment successful", so the status strings above hold.

## Tokenization

Tap-only completion needs Xsolla Tokenization, a partner feature. Ask a mentor
at 10:15. Whatever the answer, write it in the README under "What is real and
what is mocked".

## Webhooks, not today

Xsolla signs webhooks as `Authorization: Signature sha1(raw_body + secret)`.
Needs a public URL, which needs a tunnel, which needs venue WiFi to cooperate.
Polling covers the demo. The signature check is a pure function with tests and
may land as a bonus at hour 6, without a route.
