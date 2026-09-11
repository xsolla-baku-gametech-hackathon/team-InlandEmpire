//! The smallest game: wait for a tap, buy 500 gems, print the outcome.
//!
//! Run `make demo` (or the server and `tappad-bridge --fake` by hand), then:
//! `cargo run -p tappad-sdk --example tap_to_gems`

use tappad_sdk::{Config, PurchaseResponse, Sku, TapPad};

#[tokio::main]
async fn main() -> Result<(), tappad_sdk::SdkError> {
    let mut tappad = TapPad::connect(&Config::default())?;

    let items = tappad.catalog().await?;
    println!("shop sells:");
    for item in &items {
        println!("  {} {} {}", item.sku, item.price, item.currency);
    }

    println!("tap a card...");
    let uid = tappad.next_tap().await;
    println!("card {uid} tapped, buying gems_500");

    match tappad.buy(uid, Sku::new("gems_500")).await? {
        PurchaseResponse::Approved {
            order_id,
            receipt_id,
        } => {
            println!("paid on the spot, order {order_id}, receipt {receipt_id}");
        }
        PurchaseResponse::Declined { reason } => {
            println!("declined: {}", reason.message());
        }
        PurchaseResponse::PendingPayment {
            order_id,
            checkout_url,
        } => {
            println!("open this checkout in the game window:\n  {checkout_url}");
            let state = tappad.wait_for_payment(order_id).await?;
            if state.is_success() {
                println!("order {order_id} paid, grant the gems");
            } else {
                println!("order {order_id} ended {state:?}, nothing granted");
            }
        }
    }
    Ok(())
}
