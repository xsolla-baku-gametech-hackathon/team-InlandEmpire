# tappad-protocol

## 1. What this crate is

Every TapPad part talks JSON: the pad prints it over serial, the bridge
forwards it over WebSocket, the game sends it to the server and reads the
answer. This crate holds the Rust types for those messages and nothing else.
The bridge, the server and the game all import it, so a field name exists in
one place and cannot drift. The firmware is C++ and writes the same shapes by
hand; the wire tests in `src/tests.rs` are what it has to match.

Files: `src/lib.rs` (the types) and `src/tests.rs` (the tests).

## 2. The messages

| Message | From → to | Transport | Rust type | JSON example |
|---|---|---|---|---|
| ready | pad → bridge → game | serial line, then WebSocket frame | `PadEvent::Ready` | `{"event":"ready","firmware":"0.1.0"}` |
| tap | pad → bridge → game | serial line, then WebSocket frame | `PadEvent::Tap` | `{"event":"tap","uid":"04A3B2C1"}` |
| error | pad → bridge → game | serial line, then WebSocket frame | `PadEvent::Error` | `{"event":"error","message":"reader timeout"}` |
| purchase request | game → server | `POST /purchase` body | `PurchaseRequest` | `{"uid":"04A3B2C1","sku":"gems_500"}` |
| pending payment | server → game | HTTP 200 body | `PurchaseResponse::PendingPayment` | `{"status":"pending_payment","order_id":12345,"checkout_url":"https://..."}` |
| approved | server → game | HTTP 200 body | `PurchaseResponse::Approved` | `{"status":"approved","order_id":7,"receipt_id":"rcpt-000007"}` |
| declined | server → game | HTTP 200 body | `PurchaseResponse::Declined` | `{"status":"declined","reason":"limit_exceeded"}` |
| order status | server → game | `GET /orders/{id}` body | `OrderStatus` | `{"order_id":12345,"state":"paid"}` |
| error body | server → game | any non-2xx body | `ErrorBody` | `{"error":"provider failed"}` |

Two constants say where things listen: `BRIDGE_WS_ADDR` is `127.0.0.1:8765`,
`SERVER_ADDR` is `127.0.0.1:8080`.

## 3. Reading lib.rs

Open `src/lib.rs` and follow along from the top.

### The file comment and imports

```rust
//! Shared message types. The JSON shapes here are the contract between
//! firmware, bridge, server and game; see `docs/protocol.md`.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
```

`//!` is a doc comment for the whole file. `///` (you will see it everywhere
below) is a doc comment for the item right under it. `cargo doc` turns both
into HTML, and our lints fail the build if a public item has none.

`use` is `import`. `std` is the standard library, always available. `serde`
is the crate that turns Rust values into JSON and back. Python: `import json`;
JavaScript: `JSON.stringify` and `JSON.parse`, except serde checks the shape
while it parses.

### Constants

```rust
/// Where the bridge serves WebSocket frames and the game connects.
pub const BRIDGE_WS_ADDR: &str = "127.0.0.1:8765";
```

`pub` means other crates can see it. Without `pub`, an item is private to
this file. `const` is a compile-time constant. `&str` is the type of a string
that lives in the program itself and is only read, never changed. Think of it
as a read-only view onto text. `String` (later) is the owned, growable kind,
like a JavaScript string you built at runtime.

### `Cents`, the first newtype

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Cents(pub u64);
```

A **newtype** is a struct with exactly one field and no name for it. `Cents`
is a `u64` (an unsigned 64-bit integer) that the compiler refuses to mix up
with any other `u64`. You cannot pass an order id where cents are expected.
Python and JavaScript have no cheap way to do this; you would wrap it in a
class and pay for it at runtime. In Rust the wrapper costs nothing.

`pub u64` makes the inner number public, so `Cents(499)` builds one and
`c.0` reads it back. We do that for `Cents` and `OrderId` because any number
is a valid value. `CardUid` below keeps its field private because not every
string is a valid UID.

`#[derive(...)]` asks the compiler to write standard behaviour for us. Each
name is a **trait**, which is Rust's word for an interface. What each one gives
us here:

| Derive | What it gives | Why we want it |
|---|---|---|
| `Debug` | `{:?}` printing, like `repr()` | test failure messages |
| `Clone` | `.clone()` makes an explicit copy | needed by serde and callers |
| `Copy` | the value is copied on assignment, no `.clone()` needed | it is one integer, copying is free |
| `PartialEq`, `Eq` | `==` works | `assert_eq!` in tests |
| `PartialOrd`, `Ord` | `<` and sorting work | comparing a price to a limit |
| `Hash` | can be a `HashMap` key | a registry keyed by card |
| `Serialize` | value → JSON | sending |
| `Deserialize` | JSON → value | receiving |

`Copy` versus `Clone`: a `Copy` type is duplicated silently, like a number in
JavaScript. A type that is only `Clone` moves when you assign it (the old
variable is gone) unless you call `.clone()`. Small, plain values are `Copy`;
anything holding a `String` cannot be, because copying text is real work.

serde on a newtype writes the inner value alone, so `Cents(499)` is `499` on
the wire, not `{"0":499}`.

Why not `f64`? `4.99` has no exact binary representation, and adding floats
drifts. Integer cents are exact. serde also rejects `4.99`, `500.0` and `-1`
for a `u64` without us writing a line, and the tests pin that.

```rust
impl Cents {
    /// Adds two amounts, or returns `None` if the sum does not fit in `u64`.
    #[must_use]
    pub fn checked_add(self, other: Cents) -> Option<Cents> {
        self.0.checked_add(other.0).map(Cents)
    }
}
```

`impl Cents { ... }` is where methods live; it is the class body, separate
from the field list. `self` is the receiver. Taking `self` by value is fine
because `Cents` is `Copy`. `Option<Cents>` is either `Some(value)` or `None`;
it is how Rust says "maybe". There is no `null`. `checked_add` on `u64`
returns `None` on overflow instead of wrapping around, and `.map(Cents)`
rewraps the number in our type if it is there. `#[must_use]` makes the
compiler warn if a caller ignores the result.

```rust
impl fmt::Display for Cents {
    /// Prints `499` as `4.99`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}", self.0 / 100, self.0 % 100)
    }
}
```

`impl Trait for Type` implements an interface. `Display` is what `{}` in
`format!` and `.to_string()` use, like `__str__` or `toString()`. `&self`
borrows the value instead of taking it, which is what almost every method
does: the caller keeps the value. `{:02}` pads to two digits, so `5` prints
`0.05`.

### `UidError` and thiserror

```rust
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UidError {
    /// A character that is not a hex digit was found after removing separators.
    #[error("card uid contains a character that is not a hex digit")]
    NotHex,
    /// The number of hex digits does not match a 4, 7 or 10 byte UID.
    #[error("card uid has {0} hex digits, expected 8, 14 or 20")]
    BadLength(usize),
}
```

An `enum` is a fixed set of variants. Unlike enums in most languages, a
variant can carry data: `BadLength(usize)` carries the digit count, `NotHex`
carries nothing. Rust functions do not throw; they return
`Result<T, E>`, which is `Ok(value)` or `Err(error)`, and the error is a
normal value like this enum. `thiserror::Error` plus `#[error("...")]` writes
the human-readable message for each variant, so `err.to_string()` works.
`{0}` in the message is the first field of the variant.

### `CardUid`

```rust
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CardUid(String);
```

The field is private, so the only way to get a `CardUid` is through parsing,
and parsing enforces the rules. `#[serde(try_from = "String", into = "String")]`
tells serde: to read one, first read a plain string, then run our `TryFrom`
which can fail; to write one, convert it into a string. This is how the UID
rules also apply inside JSON without any extra code in the message types.

```rust
impl FromStr for CardUid {
    type Err = UidError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        let digits: String = raw
            .chars()
            .filter(|c| !matches!(c, ':' | '-' | ' '))
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(UidError::NotHex);
        }
        match digits.len() {
            8 | 14 | 20 => Ok(CardUid(digits)),
            n => Err(UidError::BadLength(n)),
        }
    }
}
```

`FromStr` is the trait behind `"04:a1:b2:c3".parse::<CardUid>()`. `Self` is
the type being implemented, here `CardUid`. The chain `.chars().filter(...)
.map(...).collect()` is the same idea as JavaScript's
`[...s].filter(...).map(...).join("")`; `|c| ...` is a closure, an arrow
function. `match` is a `switch` that must cover every case: `8 | 14 | 20`
matches three values, `n =>` catches everything else and binds it. The
compiler refuses a `match` with a missing case, which is why we use enums for
state.

The order of the two checks matters. Non-hex characters are reported as
`NotHex` even when the length is also wrong, and `04_A1_B2_C3` is `NotHex`
because `_` is not a separator we accept.

```rust
impl TryFrom<String> for CardUid {
    type Error = UidError;

    fn try_from(raw: String) -> Result<Self, Self::Error> {
        raw.parse()
    }
}

impl From<CardUid> for String {
    fn from(uid: CardUid) -> Self {
        uid.0
    }
}
```

These two are what the `#[serde(try_from, into)]` line above calls. The
first just forwards to `parse`. The second unwraps the inner string;
`uid.0` is the unnamed field.

`as_str(&self) -> &str` returns a borrowed view of the text, no copy. You
will see the same three lines on `Sku` and `ReceiptId`.

### `Sku`, `OrderId`, `ReceiptId`

```rust
impl Sku {
    /// Wraps a catalogue id.
    pub fn new(sku: impl Into<String>) -> Self {
        Sku(sku.into())
    }
```

`impl Into<String>` means "anything that can become a `String`", so
`Sku::new("gems_500")` and `Sku::new(some_string)` both work. `Sku` and
`ReceiptId` have no validation rule, so they get a `new` instead of a parser
and keep the field private only so the type stays a type, not a bare string.

```rust
pub struct OrderId(pub u64);
```

`OrderId` is an integer because Xsolla returns integers and `docs/protocol.md`
pins that. One thing to know for the game: JavaScript numbers lose precision
above 2^53. Xsolla order ids are far below that today, so `Number` is fine.

### `PadEvent`, an enum on the wire

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum PadEvent {
    /// The firmware booted and the reader answers.
    Ready {
        /// Firmware version string, for the log.
        firmware: String,
    },
```

`#[serde(tag = "event")]` writes the variant name into a field called
`event` and reads it back from there, so the JSON is
`{"event":"tap","uid":"..."}` rather than nested. `rename_all = "snake_case"`
turns `Ready` into `ready`. A variant with `{ ... }` carries named fields,
like a small struct. This is a tagged union in TypeScript terms. serde ignores
JSON fields it does not know, so the firmware can add a debug field without
breaking anything. An unknown `event` value is an error.

### `LineError` and `parse_line`

```rust
#[derive(Debug, thiserror::Error)]
pub enum LineError {
    /// The line was empty after trimming whitespace.
    #[error("empty line")]
    Empty,
    /// The line was not the JSON of a known event. Boot noise from the chip,
    /// unknown `event` values and bad UIDs all land here.
    #[error("not a pad event: {0}")]
    Json(#[from] serde_json::Error),
}
```

`#[from]` makes `LineError::Json` buildable from a `serde_json::Error`
automatically. That is what lets the `?` below work.

```rust
pub fn parse_line(line: &str) -> Result<PadEvent, LineError> {
    let line = line.trim();
    if line.is_empty() {
        return Err(LineError::Empty);
    }
    Ok(serde_json::from_str(line)?)
}
```

`let line = line.trim();` shadows the parameter with the trimmed view; this
is normal Rust. `?` means: if this call returned `Err`, convert it into our
error type and return it now; otherwise unwrap the `Ok`. It replaces a whole
`try/except` with one character. The doc comment above the function has a
`# Errors` section; the pedantic lints require one on every function that
returns `Result`.

### `PurchaseRequest`, `DeclineReason`, `PurchaseResponse`

```rust
pub struct PurchaseRequest {
    /// The card that was tapped.
    pub uid: CardUid,
    /// The item the player clicked Buy on.
    pub sku: Sku,
}
```

A plain struct with named public fields. Field names are the JSON keys.
Because `uid` is a `CardUid`, a purchase request with a bad UID fails to
parse before any server code runs.

```rust
impl DeclineReason {
    /// Every reason, so a test can prove each one has player-facing text.
    pub const ALL: [DeclineReason; 4] = [
        DeclineReason::UnknownCard,
        DeclineReason::LimitExceeded,
        DeclineReason::InsufficientFunds,
        DeclineReason::UnknownSku,
    ];

    /// The sentence the game shows the player. Lives here so it is typed once.
    #[must_use]
    pub fn message(self) -> &'static str {
        match self {
            DeclineReason::UnknownCard => "This card is not registered.",
            DeclineReason::LimitExceeded => "Over this card's spending limit.",
            DeclineReason::InsufficientFunds => "Not enough funds on this card.",
            DeclineReason::UnknownSku => "This item does not exist.",
        }
    }
}
```

`[DeclineReason; 4]` is an array of exactly four. `&'static str` is text
baked into the binary that lives for the whole program, which is what a
string literal is. The `match` has no catch-all arm on purpose: add a fifth
reason and the compiler points here until you write its sentence. The wire
carries only the reason; the sentence stays on the game side, so wording can
change without a protocol change.

```rust
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PurchaseResponse {
```

Same tagged-enum trick as `PadEvent`, keyed on `status`. Why is a decline
HTTP 200? Because the server did its job and gave a business answer. Non-2xx
is for when the server or Xsolla broke, and then the body is `ErrorBody`.
The game switches on `status`, not on the HTTP code.

```rust
impl PurchaseResponse {
    /// Builds the declined answer for a reason.
    #[must_use]
    pub fn declined(reason: DeclineReason) -> Self {
        PurchaseResponse::Declined { reason }
    }
}
```

A function in an `impl` block without `self` is a static method, called as
`PurchaseResponse::declined(...)`. `Declined { reason }` is shorthand for
`Declined { reason: reason }`, same as JavaScript object shorthand.

### `OrderState` and `OrderStatus`

```rust
impl OrderState {
    /// True when the state will not change again, so the game can stop polling.
    #[must_use]
    pub fn is_final(self) -> bool {
        !matches!(self, OrderState::New)
    }

    /// True when the player should get the item.
    #[must_use]
    pub fn is_success(self) -> bool {
        matches!(self, OrderState::Paid | OrderState::Done)
    }
}
```

`matches!(value, pattern)` is a one-line `match` that returns a bool.
The game polls `GET /orders/{id}` every 800 ms until `is_final()` is true,
then grants gems if `is_success()` is true and shows "Payment was cancelled"
otherwise.

### `ErrorBody` and the test module

```rust
pub struct ErrorBody {
    /// What went wrong, in words safe to show in a log.
    pub error: String,
}

#[cfg(test)]
mod tests;
```

`#[cfg(test)]` compiles the next item only when running tests. `mod tests;`
with a semicolon says "the module body is in `tests.rs` next to this file".
So the tests are part of the crate, as CLAUDE.md asks, but live in their own
file so a diff to the tests never mixes with a diff to the code.

## 4. Reading the tests

Open `src/tests.rs`. Because it is a module inside the crate, it starts with
`use super::*;`, which imports everything from `lib.rs`, private items
included.

```rust
#![allow(clippy::unwrap_used)]
```

The workspace forbids `unwrap` in library code because a failed unwrap
crashes the program. In a test a crash is exactly what we want on failure,
so this one line, at the top of this one file, allows it.

```rust
#[test]
fn wire_pad_tap() {
    pins(
        &PadEvent::Tap {
            uid: uid("04A3B2C1"),
        },
        json!({"event":"tap","uid":"04A3B2C1"}),
    );
}
```

`#[test]` marks a function as a test; `cargo test` finds them all. `json!`
comes from `serde_json` and builds a JSON value from a literal, like writing
an object in JavaScript. `pins` is our helper at the top of the file: it
serialises the value, compares it to the pinned JSON as parsed values (so key
order does not matter), then parses the pinned JSON back and expects the same
Rust value. Every message has one of these.

`assert_eq!(a, b)` fails the test and prints both sides when they differ.
`assert!(cond, "message {x:?}")` fails with a message; `{x:?}` prints the
variable `x` in `Debug` form inside the string.

Run everything:

```
cargo test -p tappad-protocol
```

Run one test, or every test whose name contains a word:

```
cargo test -p tappad-protocol wire_pad_tap
cargo test -p tappad-protocol uid_
```

## 5. Recipe: adding or changing a message

1. Agree the JSON with whoever sends and receives it, and write it into
   `docs/protocol.md`.
2. In `src/tests.rs`, add a `wire_...` test that calls `pins` with the Rust
   value you wish existed and the exact JSON. Run it. It fails to compile;
   that is the red state.
3. In `src/lib.rs`, add the type. Copy the nearest existing one: a newtype
   for an id, a struct for a body, a tagged enum for a message with variants.
   Put a `///` line on the type and on every field.
4. If a field has rules (a format, a range), give it its own newtype with a
   `FromStr` or a `new`, and add a behaviour test for the rule, like the
   `uid_...` tests.
5. Run `cargo fmt --all`, then
   `cargo clippy --workspace --all-targets -- -D warnings`, then
   `cargo test --workspace`. Fix until all three are clean.
6. Commit the test and the type in two small commits, `test(protocol): ...`
   then `feat(protocol): ...`.
7. Tell the teammates whose code reads or writes that message: H for
   anything on the serial line, A for anything the server sends or receives,
   B for anything the game sends or receives.

## 6. Compiler errors you will probably see here

**`error[E0308]: mismatched types` ... `expected 'String', found '&str'`**
You wrote `firmware: "0.1.0"` in a struct that wants a `String`. A literal is
a `&str`. Fix: `"0.1.0".to_string()`.

**`error[E0382]: borrow of moved value: 'expected'`**
You used a non-`Copy` value twice, for example `assert_eq!(x, expected)`
twice in a row with a `PadEvent`. The first use moved it. Fix: compare against
`expected.clone()` the first time, or make the second use the last one. (The
test `line_ignores_crlf_and_whitespace` works because `assert_eq!` only
borrows its arguments.)

**`error[E0599]: no method named 'len' found for struct 'CardUid'`**
Newtypes do not inherit the methods of what they wrap. Fix: go through the
accessor, `uid.as_str().len()`.

**`error[E0004]: non-exhaustive patterns: 'DeclineReason::NewReason' not covered`**
You added an enum variant and a `match` somewhere does not handle it. Fix:
add an arm for it in the place the error points to, usually `message`.

**`error: unused 'Result' that must be used`** or **`unused return value of 'checked_add' that must be used`**
You called something that can fail or return a value and dropped the result.
Fix: handle it with `?`, `match`, or `let value = ...`.

**`error: docs for function returning 'Result' missing '# Errors' section`**
The pedantic lint set requires a `# Errors` heading in the doc comment of
every `pub fn` that returns `Result`. Fix: copy the shape from `parse_line`.

**`error: missing documentation for a struct field`**
Every `pub` item and every field of a public struct or enum needs a `///`
line. Fix: write one line that says what the field is.
