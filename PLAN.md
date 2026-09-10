# TapPad, the plan

Tap a card on a USB pad, pay inside the game window, get the item.
The pitch: desktop games lose the sale at checkout because the player leaves the
game for a browser and a card form. Mobile converts better because it is one tap.
TapPad gives desktop the same one tap.

![architecture](docs/architecture.png)

## Onboarding

```
git clone https://github.com/xsolla-baku-gametech-hackathon/team-InlandEmpire.git
cd team-InlandEmpire && git checkout dev
```

Then everyone runs `claude` from the repo root. The repo `CLAUDE.md` and
`.claude/skills/` load on their own.

The demo-ready branch is `main`, not `master`: the org template created the
repo with `main` and we kept it.

## What we build

Five parts. Four are ours. Follow the numbers on the diagram for one purchase.

| Part | Language | Job | Owner |
|---|---|---|---|
| Pad firmware | C++ (Arduino) | Read card UID, print one JSON line over USB serial | H |
| `tappad-bridge` | Rust | Read serial lines, forward to `ws://127.0.0.1:8765`. `--fake` emits a tap every 4 s | C |
| `tappad-server` | Rust, axum, `:8080` | `POST /purchase` checks card limit, asks Xsolla for an order. `GET /orders/{id}` reports paid or not | AB |
| `tappad-game` | Tauri, Rust + one HTML page | Shop, gem counter, checkout in an iframe, polls until paid | AB |
| `tappad-protocol` | Rust | Shared types so field names cannot drift | C |

Xsolla is external. Two calls, both from the server. Details in `docs/xsolla.md`.
Messages between parts in `docs/protocol.md`.

Rust never calls the C++. The chip prints text over USB. The bridge reads it.
The JSON shape is the whole contract.

## Decisions already made

- Tauri for the game. Not macroquad, not wry on a thread. The checkout must
  render inside the game window, and Tauri gives that for free.
- Arduino C++ for the firmware. Not Rust on ESP32. The toolchain is the risk,
  not the code.
- Poll `GET /orders/{id}` every 0.8 s. No webhooks, no tunnel. The
  `verify_signature` module and its tests may land at the end as a bonus.
- Serial port is a required flag on the bridge. On a Mac it looks like
  `/dev/cu.usbserial-XXXX`, on Linux `/dev/ttyUSB0`.
- `PaymentProvider` trait with `Xsolla` and `Mock`. `TAPPAD_PROVIDER=mock` is
  the default so the whole loop runs offline.
- Money is `Cents(u64)`. Never a float.
- The Xsolla API key lives only in the server process, from a git-ignored `.env`.
- Tap-only completion needs Xsolla Tokenization, a partner feature. Ask a
  mentor at 10:15. If no, the demo is tap creates the order, one click on the
  saved test card. Say what is mocked out loud, in the README and on stage.

## The demo

1. Dad card taps. Checkout appears in the game. Confirm with the test card. Gems go up.
2. Kid card taps the same item. "Over this card's spending limit." Xsolla is never called.

Two taps, one minute. Record it at hour 6 as the backup video.

## Team

Three devs. Two of them, AB, work as a pair. C works alone. One hardware
engineer H. One presenter P. Everyone uses Claude Code from the repo root so
the same rules apply to every commit.

- AB own the money path end to end: Xsolla Publisher Account, the item, the
  API key, the server, and the game that shows the checkout. One of them
  holds the console login; the other never needs it. They split by hour, not
  by crate: server first, game once `/purchase` answers.
- C owns the protocol crate, the bridge, tests and CI, and merges `dev` into
  `main` at each checkpoint.
- H owns the firmware alone until the pad is plugged into the bridge.
- P owns the deck from hour 1, the demo script, and the backup video.

## The clock

Build opens Sept 10 at 10:00. Code freeze Sept 11 at 12:00. Decks to organisers
Sept 11 at 14:00. Pitches 15:00 to 16:40, 3 minutes plus 2 minutes Q&A.

| Hour | AB | C | H | P |
|---|---|---|---|---|
| 1 | Server skeleton, mock provider, registry. `curl` returns approved and declined. Tauri window opens with three items and a gem counter | Protocol crate with tests. Workspace, CI green on `dev` | Firmware prints tap lines in the serial monitor | Problem slide, the person who has it |
| 2 | Server routes stable, error mapping. WebSocket listener in the page, tap shows on screen | Bridge with `--fake` | Debounce, LED, second card | Demo script |
| 3 | Xsolla client: create token, get order. `POST /purchase` from the page, declined path shown | Bridge on the real serial port | Plug pad into the bridge with C | Deck v1 |
| 4 | Real order created in sandbox. iframe checkout, polling, gems on paid | Integration test: fake tap to gems, mock | Enclosure, cable strain | Talk track timed |
| 5 | Real sandbox payment through the game. Result screen, error text for every decline | Merge to `main`, tag `v0.2.0-sandbox` | Full loop on real pad | Q&A prep |
| 6 | Polish shop, remove debug. Bonus: `verify_signature` module with tests | README truthful, `docs/` current | Spare pad flashed | Backup video recorded |
| 7 | Review C's crates | Clippy clean everywhere, tag `v0.3.0-demo`, review AB's crates | Rehearse the tap | Deck v2 |
| 8 | Rehearse | Freeze `main` | Rehearse | Submit deck |

Checkpoints, each one is a merge to `main` and a tag:

| When | Must be true | Tag |
|---|---|---|
| 13:00 | fake tap gives gems with the mock provider | `v0.1.0-mock` |
| 15:30 | real Xsolla sandbox payment goes through the game | `v0.2.0-sandbox` |
| 18:00 | real pad, Dad pays, Kid declined, backup video recorded | `v0.3.0-demo` |
| Sept 11 12:00 | freeze | `v1.0.0` |

The 13:00 rule: if the real Xsolla payment is not working by 13:00 on Sept 11,
the demo runs on the mock provider and the Xsolla call becomes a slide. Decide
then, not at 15:00.

## Cut list

Not in this build, no discussion: webhook route and tunnel, LED ring if the
reader is not working by 12:00, Rust firmware, macroquad, a terminal dashboard,
the docs-build CI step, a second game, a login system.

## Prizes we aim at

Best Project, Best Code, Best Idea. All three want the same thing: a working
demo and code an Xsolla engineer can read in five minutes. GitHub Star is a
commit count. Commit honestly every time something works and leave it there.

## Pitch skeleton

The organisers' four questions, answered in order. Who has the problem: the gaming
lounge or LAN cafe operator whose players pay cash at the counter and never buy
in-game. What is the demo: two taps. Why has nobody built it: tap-to-pay
hardware only just became cheap and Xsolla only just made server-side tokens
easy. Why all of us: hardware, backend, desktop app, and someone who can sell it.

## Skills to install in the repo, hour 0

```
npx skills add apollographql/skills@rust-best-practices
npx skills add affaan-m/ecc@rust-testing
npx skills add nodnarbnitram/claude-code-extensions@tauri-v2
npx skills add dchuk/claude-code-tauri-skills@calling-rust-from-tauri-frontend
npx skills add github/awesome-copilot@conventional-commit
```

Our own `/checkpoint` skill is already in `.claude/skills/`. Run it before
every push.
