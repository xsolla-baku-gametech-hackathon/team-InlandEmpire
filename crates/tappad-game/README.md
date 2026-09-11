# tappad-game

Tauri desktop app: a shop page with a gem counter. Buy, tap a card, the
server answers, gems land. With the Xsolla provider the checkout page opens
in an iframe inside the same window and the game polls the order until it
is paid.

UI is plain HTML and ES modules in `ui/`, no framework. State machine in
`ui/shop.js`, tests in `ui-tests/`.

```
cargo tauri dev                                  # from this folder
node --test "ui-tests/*.test.mjs"
```
