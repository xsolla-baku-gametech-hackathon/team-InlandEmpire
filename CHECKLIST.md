# Checklist

Tick a box when the thing runs, not when the code is written. Claude updates
this file when it has verified the condition. Owner initials in brackets.

## Before kickoff (accounts, installs, reading only)

- [ ] Team repo created, `dev` branch exists, this folder is on it [C]
- [ ] Everyone has `claude` running from the repo root
- [x] Rust stable, `cargo tauri --version` prints [B]
- [x] `cargo tauri dev` opens a blank window on the game laptop [B]
- [ ] Xsolla Publisher Account, sandbox project, project id noted [A]
- [ ] Server API key created in Publisher Account [A]
- [ ] Item `gems_500` in the catalogue, price 4.99 USD [A]
- [ ] The `curl` in `docs/xsolla.md` returns a token and an order_id [A]
- [ ] Test bank card numbers copied from Xsolla docs [A]
- [ ] Arduino IDE flashes the ESP32, PN532 prints a card UID [H]
- [ ] Two NFC cards, UIDs written down [H]
- [ ] Skills installed in the repo: `npx skills add apollographql/skills@rust-best-practices` and `npx skills add nodnarbnitram/claude-code-extensions@tauri-v2` [C]
- [ ] Deck template opened, problem slide drafted [P]

## Hour 1

- [ ] `tappad-protocol` builds, tests pass [C]
- [ ] CI green on `dev` [C]
- [ ] `POST /purchase` with mock returns `approved` for Dad [A]
- [ ] `POST /purchase` with mock returns `declined limit_exceeded` for Kid [A]
- [x] Tauri window shows shop and gem counter [B]
- [ ] Firmware prints `{"event":"tap","uid":"..."}` per tap [H]
- [ ] Mentor asked about Tokenization, answer written in README [A]

## Hours 2 to 3

- [ ] `tappad-bridge --fake` broadcasts taps on `:8765` [C]
- [x] Game shows "tap detected" from the fake bridge [B]
- [ ] Game calls `POST /purchase`, shows declined text [B]
- [ ] Xsolla client creates a real sandbox order [A]
- [ ] Bridge reads the real serial port [C]

## Checkpoint 13:00, tag `v0.1.0-mock`

- [ ] Fake tap gives gems end to end with the mock provider
- [ ] `dev` merged to `main`, tagged [C]

## Hours 4 to 5

- [ ] iframe shows the sandbox checkout inside the game [B]
- [ ] Polling flips to paid, gems granted [B]
- [ ] Real pad tap starts a real sandbox purchase [A B C H]
- [ ] Integration test: fake tap to gems, mock provider [C]

## Checkpoint 15:30, tag `v0.2.0-sandbox`

- [ ] Real Xsolla sandbox payment through the game
- [ ] `dev` merged to `main`, tagged [C]

## Hours 6 to 7

- [ ] Backup video recorded, two taps, under one minute [P]
- [ ] README says what is real and what is mocked, and it is true [C]
- [ ] `docs/protocol.md` matches the code [C]
- [ ] Clippy clean on every crate, no `unwrap` outside tests [all]
- [ ] Each crate reviewed by someone who did not write it
- [ ] Bonus only: `verify_signature` module with tests, no route [A]

## Checkpoint 18:00, tag `v0.3.0-demo`

- [ ] Dad pays, Kid declined, on the real pad
- [ ] `dev` merged to `main`, tagged [C]

## Sept 11

- [ ] 10:00 full rehearsal on the presentation laptop, real pad
- [ ] 12:00 freeze, tag `v1.0.0`, nobody pushes after this [C]
- [ ] 13:00 decision: real Xsolla or mock for the demo
- [ ] 13:30 deck submitted to organisers [P]
- [ ] Presentation laptop: server, bridge, game start with one script
- [ ] Spare pad and spare cards in the bag [H]
