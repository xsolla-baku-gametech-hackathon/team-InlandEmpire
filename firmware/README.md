# Pad firmware

ESP32 + RC522 (MFRC522) over SPI, two piezo buzzers. Arduino C++, one sketch:
`tappad_pad/tappad_pad.ino`.

## Wiring

| RC522 | ESP32 |
|---|---|
| SDA / SS | GPIO 5 |
| SCK | GPIO 18 |
| MOSI | GPIO 23 |
| MISO | GPIO 19 |
| RST | GPIO 27 |
| 3.3V | 3.3V |
| GND | GND |

Buzzers: GPIO 25 and GPIO 26 to one leg each, other legs to GND.

## Flash

1. Arduino IDE, board **ESP32 Dev Module**.
2. Library Manager: install **MFRC522** by GithubCommunity.
3. Open `tappad_pad/tappad_pad.ino`, upload.
4. Serial Monitor at **115200**. On boot you get one line:
   `{"event":"ready","firmware":"0.1.0"}`. Each card hold prints one line:
   `{"event":"tap","uid":"04A3B2C1"}`.

If the boot line is preceded by `{"event":"error","message":"reader not found"}`,
the RC522 is not answering on SPI: check SS, RST and the 3.3V rail.

## Contract

One JSON object per line, nothing else on the wire. Shapes are in
`docs/protocol.md`; `crates/tappad-bridge/tests/firmware_lines.rs` pins the
exact strings this sketch prints. Change them together.

The pad reports taps. It does not decide anything: no amounts, no approval
sound. The server answers `approved` or `declined`; the game shows it.

## Registering the demo cards

Hold each card, copy its `uid` from the Serial Monitor, and put the two values
into `Registry::demo()` in `crates/tappad-server/src/registry.rs` as Gold and
Starter. Until that is done both cards answer `unknown_card`.

## Run against the bridge

```
cargo run -p tappad-bridge -- --port COM3          # Windows, see Device Manager
cargo run -p tappad-bridge -- --port /dev/cu.usbserial-0001
```

The bridge logs `pad` with the parsed event for every good line and ignores
anything else.
