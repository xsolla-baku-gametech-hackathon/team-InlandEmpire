# TapPad

Tap a card on a USB pad, pay inside the game window, get the item.
Tap-to-pay for desktop games, built on Xsolla.

![architecture](docs/architecture.png)

## How it works

Player clicks Buy. Player taps a card on the pad. The pad prints the card ID
over USB. The bridge forwards it to the game. The game asks the server. The
server checks the card's spending limit and asks Xsolla for an order. The
Xsolla checkout appears inside the game window. The player confirms. The game
polls until the order is paid and grants the gems.

## Run the demo, no hardware, no Xsolla account

```
make demo                                    # server, fake bridge, then the game if `cargo tauri` exists
scripts\demo.ps1                             # same on Windows without make
```

Or by hand:

```
cargo run -p tappad-server                   # TAPPAD_PROVIDER=mock by default
cargo run -p tappad-bridge -- --fake
cargo tauri dev                              # from crates/tappad-game
```

## Run against the Xsolla sandbox

```
cp .env.example .env                         # fill in project id and API key, TAPPAD_PROVIDER=xsolla
make demo PORT=/dev/cu.usbserial-XXXX        # or scripts\demo.ps1 COM3 on Windows
```

## Test

```
cargo test --workspace
```

## Layout

| Path | What |
|---|---|
| `crates/tappad-protocol` | Shared message types |
| `crates/tappad-server` | Card registry, Xsolla client, `PaymentProvider` |
| `crates/tappad-bridge` | Serial to WebSocket |
| `crates/tappad-game` | Tauri desktop app |
| `firmware/` | Arduino sketch for ESP32 + RC522 |
| `docs/` | Protocol, Xsolla setup, architecture |

## What is real and what is mocked

Real: the pad, the card read, order creation in the Xsolla sandbox, the
sandbox checkout inside the game, order status polling.

Mocked: tap-only completion. That needs Xsolla Tokenization, a partner feature.
Today a tap creates the order and the player confirms with one click on the
saved test card.

## Engineering decisions

- Money is integer cents.
- One shared crate for message types so firmware, bridge, server and game
  cannot drift.
- `PaymentProvider` trait: Xsolla in production, mock for offline demo and tests.
- The API key lives only in the server process.
- Firmware is Arduino C++ because the MFRC522 library is mature there and the
  chip only prints JSON lines.

## Roadmap

USB pad today. Xsolla Tokenization for true one tap. Pad built into gaming
lounge seats. Launcher integration.

## Team

Names here.

## Hackathon

Xsolla Baku GameTech Hackathon, Sept 9–11. Organiser rules are in
[`docs/hackathon-rules.md`](docs/hackathon-rules.md), conduct in
[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
