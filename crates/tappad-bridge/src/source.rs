//! Where pad events come from. The fake source needs no hardware.

use std::time::Duration;

use tappad_protocol::{CardUid, PadEvent};
use tokio::sync::broadcast;

/// Demo cards the fake pad alternates between: Dad, then Kid.
pub const FAKE_UIDS: [&str; 2] = ["04A3B2C1", "04D4E5F6"];

/// Serialises an event to the one-line JSON the game expects.
///
/// # Errors
/// Never in practice; `PadEvent` always serialises.
pub fn to_line(event: &PadEvent) -> serde_json::Result<String> {
    serde_json::to_string(event)
}

/// Sends `ready`, then one `tap` every `interval`, forever.
///
/// # Errors
/// A demo UID that does not parse, which would be a bug in this file.
pub async fn run_fake(tx: broadcast::Sender<String>, interval: Duration) -> anyhow::Result<()> {
    let uids: Vec<CardUid> = FAKE_UIDS
        .iter()
        .map(|uid| uid.parse())
        .collect::<Result<_, _>>()?;
    let ready = PadEvent::Ready {
        firmware: "fake".into(),
    };
    tracing::info!("fake pad ready, one tap every {interval:?}");
    let _ = tx.send(to_line(&ready)?);
    for uid in uids.iter().cycle() {
        tokio::time::sleep(interval).await;
        let event = PadEvent::Tap { uid: uid.clone() };
        tracing::info!(%uid, "fake tap");
        let _ = tx.send(to_line(&event)?);
    }
    Ok(())
}
