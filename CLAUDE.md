# TapPad repo rules

Read `PLAN.md` once. Work from `CHECKLIST.md`. Git flow is in `GIT.md`.
Xsolla facts are in `docs/xsolla.md`, message shapes in `docs/protocol.md`.

## Build the smallest thing that runs

One path. Make it run, verify it ran, then the next step. Before adding a
crate, trait, generic, or config knob, ask whether today's demo needs it.
Default: no. A trait earns its place with two implementations, a generic with
two concrete types, a builder with four or more optional fields.

## Rust

- `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test --workspace` before every push. CI runs the same three.
- `unwrap`, `expect` and `panic!` only in tests and in `main` after startup
  config. Library code returns `Result`. `thiserror` for library errors,
  `anyhow` in binaries.
- Newtypes for IDs and money: `CardUid`, `Sku`, `OrderId`, `Cents`. Never a
  bare `String` or `f64` for those.
- Enums for state. `OrderState`, `ShopState`, `PurchaseResponse`. Match on
  them exhaustively.
- One job per crate, one job per module. `tappad-server` has no serial code,
  `tappad-bridge` has no HTTP.
- Dependencies come from the workspace `Cargo.toml`. Add one only if the
  standard library and the existing deps cannot do it.
- `tracing` for logs. `println!` only in the firmware, where serial is the
  output.

## Design

Depend on the trait, not the vendor: `PaymentProvider` is the only door to
Xsolla. `ShopState` is a state machine, transitions live in one `impl`.
Registry checks run before any network call, and a business answer such as
"limit exceeded" is a `Declined` response, never an `Err`.

## Docs

`///` on every `pub` item. One line that says what it is. A second line only
when the why is not obvious. Module-level `//!` says what the file is for in
one sentence. Comments explain a decision, not restate the code.

## Tests

Next to the code in `mod tests`. Test the rule, not the plumbing: limit
exceeded, unknown card, JSON round trip, signature rejects a tampered body.
No test hits the network.

## Secrets

`XSOLLA_API_KEY` is read once in `tappad-server` into a `SecretString`. It is
never logged, never sent to the game, never committed. `.env` is ignored,
`.env.example` has placeholders.

## Commits and the checklist

Follow `GIT.md`. Commit when something runs that did not run before. When a
box in `CHECKLIST.md` becomes true and you have seen it run, tick it in the
same commit. Commit messages name the change. They carry no tool names and no
generated-by lines.

## Honesty

`README.md` has a section "What is real and what is mocked". Keep it true at
every commit. If a feature is stubbed, the stub says so in its doc comment.
