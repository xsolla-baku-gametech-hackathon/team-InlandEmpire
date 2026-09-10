# Checklist

Tick a box when the thing runs, not when the code is written. Claude updates
this file when it has verified the condition. Owner initials in brackets.

## Before kickoff (accounts, installs, reading only)

- [ ] Team repo created, `dev` branch exists, this folder is on it [C]
- [ ] Everyone has `claude` running from the repo root
- [x] Rust stable, `cargo tauri --version` prints [AB]
- [x] `cargo tauri dev` opens a blank window on the game laptop [AB]
- [ ] Xsolla Publisher Account, sandbox project, project id noted [AB]
- [ ] Server API key created in Publisher Account [AB]
- [ ] Item `gems_500` in the catalogue, price 4.99 USD [AB]
- [ ] The `curl` in `docs/xsolla.md` returns a token and an order_id [AB]
- [ ] Test bank card numbers copied from Xsolla docs [AB]
- [ ] Arduino IDE flashes the ESP32, PN532 prints a card UID [H]
- [ ] Two NFC cards, UIDs written down [H]
- [ ] Skills installed in the repo: `npx skills add apollographql/skills@rust-best-practices` and `npx skills add nodnarbnitram/claude-code-extensions@tauri-v2` [C]
- [ ] Deck template opened, problem slide drafted [P]

## Hour 1

- [ ] `tappad-protocol` builds, tests pass [C]
- [ ] CI green on `dev` [C]
- [ ] `POST /purchase` with mock returns `approved` for Dad [AB]
- [ ] `POST /purchase` with mock returns `declined limit_exceeded` for Kid [AB]
- [ ] Tauri window shows shop and gem counter [AB]
- [ ] Firmware prints `{"event":"tap","uid":"..."}` per tap [H]
- [ ] Mentor asked about Tokenization, answer written in README [AB]

## Hours 2 to 3

- [ ] `tappad-bridge --fake` broadcasts taps on `:8765` [C]
- [ ] Game shows "tap detected" from the fake bridge [AB]
- [ ] Game calls `POST /purchase`, shows declined text [AB]
- [ ] Xsolla client creates a real sandbox order [AB]
- [ ] Bridge reads the real serial port [C]

## Checkpoint 13:00, tag `v0.1.0-mock`

- [ ] Fake tap gives gems end to end with the mock provider
- [ ] `dev` merged to `main`, tagged [C]

## Hours 4 to 5

- [ ] iframe shows the sandbox checkout inside the game [AB]
- [ ] Polling flips to paid, gems granted [AB]
- [ ] Real pad tap starts a real sandbox purchase [AB C H]
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
- [ ] Bonus only: `verify_signature` module with tests, no route [AB]

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
