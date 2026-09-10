//! Tests for the wire contract in `docs/protocol.md`. Written before the
//! implementation; the JSON pinned here is the contract, the code follows it.
#![allow(clippy::unwrap_used)]

use std::fmt::Debug;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};

use super::*;

/// Serialises `value`, compares it to the pinned JSON as `Value`s so field
/// order does not matter, then deserialises the pinned JSON back and expects
/// the same value.
fn pins<T>(value: &T, wire: Value)
where
    T: Serialize + DeserializeOwned + PartialEq + Debug,
{
    let actual = serde_json::to_value(value).unwrap();
    assert_eq!(
        actual, wire,
        "serialised JSON differs from the pinned wire shape"
    );
    let back: T = serde_json::from_value(wire).unwrap();
    assert_eq!(
        &back, value,
        "pinned JSON did not deserialise back to the same value"
    );
}

fn uid(s: &str) -> CardUid {
    s.parse().unwrap()
}

// ---------------------------------------------------------------- wire pins

#[test]
fn wire_pad_ready() {
    pins(
        &PadEvent::Ready {
            firmware: "0.1.0".to_string(),
        },
        json!({"event":"ready","firmware":"0.1.0"}),
    );
}

#[test]
fn wire_pad_tap() {
    pins(
        &PadEvent::Tap {
            uid: uid("04A3B2C1"),
        },
        json!({"event":"tap","uid":"04A3B2C1"}),
    );
}

#[test]
fn wire_pad_error() {
    pins(
        &PadEvent::Error {
            message: "reader timeout".to_string(),
        },
        json!({"event":"error","message":"reader timeout"}),
    );
}

#[test]
fn wire_purchase_request() {
    pins(
        &PurchaseRequest {
            uid: uid("04A3B2C1"),
            sku: Sku::new("gems_500"),
        },
        json!({"uid":"04A3B2C1","sku":"gems_500"}),
    );
}

#[test]
fn wire_purchase_pending_payment() {
    pins(
        &PurchaseResponse::PendingPayment {
            order_id: OrderId(12345),
            checkout_url: "https://sandbox-secure.xsolla.com/paystation4/?token=abc".to_string(),
        },
        json!({
            "status":"pending_payment",
            "order_id":12345,
            "checkout_url":"https://sandbox-secure.xsolla.com/paystation4/?token=abc"
        }),
    );
}

#[test]
fn wire_purchase_approved() {
    pins(
        &PurchaseResponse::Approved {
            order_id: OrderId(7),
            receipt_id: ReceiptId::new("rcpt-000007"),
        },
        json!({"status":"approved","order_id":7,"receipt_id":"rcpt-000007"}),
    );
}

#[test]
fn wire_purchase_declined() {
    pins(
        &PurchaseResponse::Declined {
            reason: DeclineReason::LimitExceeded,
        },
        json!({"status":"declined","reason":"limit_exceeded"}),
    );
}

#[test]
fn wire_decline_reasons() {
    pins(&DeclineReason::UnknownCard, json!("unknown_card"));
    pins(&DeclineReason::LimitExceeded, json!("limit_exceeded"));
    pins(
        &DeclineReason::InsufficientFunds,
        json!("insufficient_funds"),
    );
    pins(&DeclineReason::UnknownSku, json!("unknown_sku"));
}

#[test]
fn wire_order_status() {
    pins(
        &OrderStatus {
            order_id: OrderId(12345),
            state: OrderState::Paid,
        },
        json!({"order_id":12345,"state":"paid"}),
    );
}

#[test]
fn wire_order_states() {
    pins(&OrderState::New, json!("new"));
    pins(&OrderState::Paid, json!("paid"));
    pins(&OrderState::Done, json!("done"));
    pins(&OrderState::Canceled, json!("canceled"));
    pins(&OrderState::Expired, json!("expired"));
}

#[test]
fn wire_error_body() {
    pins(
        &ErrorBody {
            error: "provider failed".to_string(),
        },
        json!({"error":"provider failed"}),
    );
}

#[test]
fn wire_cents_is_a_bare_integer() {
    pins(&Cents(499), json!(499));
}

#[test]
fn addresses_are_pinned() {
    assert_eq!(BRIDGE_WS_ADDR, "127.0.0.1:8765");
    assert_eq!(SERVER_ADDR, "127.0.0.1:8080");
}

// -------------------------------------------------------------------- money

#[test]
fn cents_accepts_integer() {
    let c: Cents = serde_json::from_str("499").unwrap();
    assert_eq!(c, Cents(499));
}

#[test]
fn cents_rejects_fractional_float() {
    assert!(serde_json::from_str::<Cents>("4.99").is_err());
}

#[test]
fn cents_rejects_whole_float() {
    assert!(serde_json::from_str::<Cents>("500.0").is_err());
}

#[test]
fn cents_rejects_negative() {
    assert!(serde_json::from_str::<Cents>("-1").is_err());
}

#[test]
fn cents_displays_as_decimal() {
    assert_eq!(Cents(499).to_string(), "4.99");
    assert_eq!(Cents(5).to_string(), "0.05");
    assert_eq!(Cents(1000).to_string(), "10.00");
    assert_eq!(Cents(0).to_string(), "0.00");
}

#[test]
fn cents_checked_add_sums() {
    assert_eq!(Cents(1).checked_add(Cents(2)), Some(Cents(3)));
}

#[test]
fn cents_checked_add_overflow_is_none() {
    assert_eq!(Cents(u64::MAX).checked_add(Cents(1)), None);
}

// ----------------------------------------------------------------- card uid

#[test]
fn uid_normalises_case_and_separators() {
    for raw in [
        "04a1b2c3",
        "04:a1:b2:c3",
        "04-A1-b2-C3",
        "04 a1 b2 c3",
        "04A1B2C3",
    ] {
        assert_eq!(uid(raw).as_str(), "04A1B2C3", "input {raw:?}");
    }
}

#[test]
fn uid_accepts_4_7_10_bytes() {
    assert_eq!(uid("04A1B2C3").as_str().len(), 8);
    assert_eq!(uid("04A1B2C3D4E5F6").as_str().len(), 14);
    assert_eq!(uid("04A1B2C3D4E5F60708A9").as_str().len(), 20);
}

#[test]
fn uid_rejects_other_lengths() {
    for raw in [
        "",
        "04A1B2",                 // 3 bytes
        "04A1B2C3D4",             // 5 bytes
        "04A1B2C3D4E5",           // 6 bytes
        "04A1B2C3D4E5F607",       // 8 bytes
        "04A1B2C3D4E5F60708A9B",  // odd digit count
        "04A1B2C3D4E5F60708A9BB", // 11 bytes
    ] {
        let err = raw.parse::<CardUid>().unwrap_err();
        assert!(
            matches!(err, UidError::BadLength(_)),
            "input {raw:?} gave {err:?}"
        );
    }
}

#[test]
fn uid_rejects_non_hex() {
    for raw in ["04A1B2CZ", "04A1B2CG", "hello!!!", "04_A1_B2_C3"] {
        let err = raw.parse::<CardUid>().unwrap_err();
        assert!(
            matches!(err, UidError::NotHex),
            "input {raw:?} gave {err:?}"
        );
    }
}

#[test]
fn uid_rules_apply_inside_json() {
    let tap: PadEvent = serde_json::from_str(r#"{"event":"tap","uid":"04:a1:b2:c3"}"#).unwrap();
    assert_eq!(
        tap,
        PadEvent::Tap {
            uid: uid("04A1B2C3")
        }
    );

    let req: PurchaseRequest =
        serde_json::from_str(r#"{"uid":"04-a1-b2-c3","sku":"gems_500"}"#).unwrap();
    assert_eq!(req.uid.as_str(), "04A1B2C3");

    assert!(serde_json::from_str::<PadEvent>(r#"{"event":"tap","uid":"ZZZZZZZZ"}"#).is_err());
    assert!(serde_json::from_str::<PadEvent>(r#"{"event":"tap","uid":"04A1B2C3D4"}"#).is_err());
    assert!(serde_json::from_str::<PurchaseRequest>(r#"{"uid":"nope","sku":"gems_500"}"#).is_err());
}

#[test]
fn uid_serialises_normalised() {
    assert_eq!(
        serde_json::to_value(uid("04:a1:b2:c3")).unwrap(),
        json!("04A1B2C3")
    );
}

// -------------------------------------------------------------- serial line

#[test]
fn line_parses_every_event() {
    assert_eq!(
        parse_line(r#"{"event":"ready","firmware":"0.1.0"}"#).unwrap(),
        PadEvent::Ready {
            firmware: "0.1.0".to_string()
        }
    );
    assert_eq!(
        parse_line(r#"{"event":"tap","uid":"04A3B2C1"}"#).unwrap(),
        PadEvent::Tap {
            uid: uid("04A3B2C1")
        }
    );
    assert_eq!(
        parse_line(r#"{"event":"error","message":"reader timeout"}"#).unwrap(),
        PadEvent::Error {
            message: "reader timeout".to_string()
        }
    );
}

#[test]
fn line_ignores_crlf_and_whitespace() {
    let expected = PadEvent::Tap {
        uid: uid("04A3B2C1"),
    };
    assert_eq!(
        parse_line("{\"event\":\"tap\",\"uid\":\"04A3B2C1\"}\r\n").unwrap(),
        expected
    );
    assert_eq!(
        parse_line("  \t{\"event\":\"tap\",\"uid\":\"04A3B2C1\"}  \n").unwrap(),
        expected
    );
}

#[test]
fn line_tolerates_extra_fields() {
    let line = r#"{"event":"tap","uid":"04A3B2C1","rssi":-40,"debug":{"x":1}}"#;
    assert_eq!(
        parse_line(line).unwrap(),
        PadEvent::Tap {
            uid: uid("04A3B2C1")
        }
    );
}

#[test]
fn line_rejects_boot_noise() {
    for noise in [
        "",
        "\r\n",
        "ets Jul 29 2019 12:21:46",
        "rst:0x1 (POWERON_RESET),boot:0x13 (SPI_FAST_FLASH_BOOT)",
        "{}",
        "{\"event\":",
        "PN532 ready",
    ] {
        assert!(parse_line(noise).is_err(), "accepted noise {noise:?}");
    }
}

#[test]
fn line_rejects_unknown_event() {
    assert!(parse_line(r#"{"event":"boot","uid":"04A3B2C1"}"#).is_err());
    assert!(parse_line(r#"{"type":"tap","uid":"04A3B2C1"}"#).is_err());
}

#[test]
fn line_rejects_bad_uid() {
    assert!(parse_line(r#"{"event":"tap","uid":"04A3B2"}"#).is_err());
    assert!(parse_line(r#"{"event":"tap","uid":"04A3B2CQ"}"#).is_err());
    assert!(parse_line(r#"{"event":"tap"}"#).is_err());
}

// ----------------------------------------------------------------- declines

#[test]
fn decline_every_reason_has_text() {
    assert_eq!(DeclineReason::ALL.len(), 4);
    for reason in DeclineReason::ALL {
        let text = reason.message();
        assert!(
            !text.trim().is_empty(),
            "{reason:?} has no player-facing text"
        );
        assert!(
            text.ends_with('.'),
            "{reason:?} text should be a sentence: {text:?}"
        );
    }
}

#[test]
fn decline_limit_exceeded_text_is_exact() {
    assert_eq!(
        DeclineReason::LimitExceeded.message(),
        "Over this card's spending limit."
    );
}

#[test]
fn declined_constructor_matches_wire() {
    let response = PurchaseResponse::declined(DeclineReason::UnknownCard);
    assert_eq!(
        response,
        PurchaseResponse::Declined {
            reason: DeclineReason::UnknownCard
        }
    );
    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        json!({"status":"declined","reason":"unknown_card"})
    );
}

// ------------------------------------------------------------------- orders

#[test]
fn order_state_final_states_stop_polling() {
    for state in [
        OrderState::Paid,
        OrderState::Done,
        OrderState::Canceled,
        OrderState::Expired,
    ] {
        assert!(state.is_final(), "{state:?} should stop polling");
    }
}

#[test]
fn order_state_new_keeps_polling() {
    assert!(!OrderState::New.is_final());
}

#[test]
fn order_state_success_is_paid_or_done() {
    assert!(OrderState::Paid.is_success());
    assert!(OrderState::Done.is_success());
    for state in [OrderState::New, OrderState::Canceled, OrderState::Expired] {
        assert!(!state.is_success(), "{state:?} is not a success");
    }
}

#[test]
fn order_status_rejects_unknown_state() {
    assert!(serde_json::from_str::<OrderStatus>(r#"{"order_id":1,"state":"refunded"}"#).is_err());
}
