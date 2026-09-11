# Slide notes

Source text for the deck. Numbers measured on 2026-09-10 against the Xsolla
sandbox, project 315338.

## What is real

- The pad, the card read, the card registry: Gold $50, All The Things (blue fob) $10, Starter $1,
  Blocked $0 per tap, a $500 cap per card for the run, all USD.
- Order creation through the Xsolla Store API. Every sandbox purchase is a
  real Xsolla order with a real order id, visible in Publisher Account.
- Pay Station checkout inside the game window.
- Order status polling until Xsolla reports `done`.

This is the production code path. Nothing is swapped out for the demo except
who presses Pay.

## What is a stand-in

Who presses Pay. Production charges the card the pad identified through
Xsolla Tokenization, a partner feature we do not have yet. With it the
checkout page disappears and a tap settles in the time Xsolla takes to
confirm the order.

Today the server pays each sandbox order itself through a headless checkout.

## Timing, one sandbox purchase

| Step | Time |
|---|---|
| Create order | 0.6 s |
| Load and fill the checkout page (stand-in) | 23 s |
| Pay click to Xsolla `done` | 4 s |
| Total | 27 s |

Production: 27 s minus the checkout page, about 5 s.

## Demo plan

Live taps on the mock provider only. The Gold card, white with the Xsolla
sticker, buys 500 gems. The Blocked card, white with the All The Things
sticker, is refused: it is enrolled with a zero spending limit, so the shop
says "Card declined." Instant, works without venue WiFi.

The Xsolla integration is shown on a slide, not live: a screenshot of a
sandbox order in Publisher Account next to the timing table above, and the
line "production is this minus the checkout page".

## Roadmap

USB pad today. Tokenization for true one tap. Pad built into gaming lounge
seats. Launcher integration.
