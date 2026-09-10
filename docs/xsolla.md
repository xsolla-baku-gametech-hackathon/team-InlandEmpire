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

1. Sign up at publisher.xsolla.com, create a project. Project id is in the URL.
2. Project settings, API keys: create a server key. Put it in `.env` only.
3. Store, Virtual items: add `gems_500`, price 4.99 USD.
4. Run the curl above. A token comes back or the day starts badly.

## Tokenization

Tap-only completion needs Xsolla Tokenization, a partner feature. Ask a mentor
at 10:15. Whatever the answer, write it in the README under "What is real and
what is mocked".

## Webhooks, not today

Xsolla signs webhooks as `Authorization: Signature sha1(raw_body + secret)`.
Needs a public URL, which needs a tunnel, which needs venue WiFi to cooperate.
Polling covers the demo. The signature check is a pure function with tests and
may land as a bonus at hour 6, without a route.
