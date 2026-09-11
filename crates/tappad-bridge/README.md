# tappad-bridge

Reads the pad's USB serial port line by line and forwards every line to a
local WebSocket on `ws://127.0.0.1:8765`. The game subscribes there.

```
cargo run -p tappad-bridge -- --port /dev/cu.usbserial-XXXX   # real pad
cargo run -p tappad-bridge -- --fake                           # a tap every 5 s, no hardware
```
