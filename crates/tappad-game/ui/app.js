// Wires the page to the bridge. Hour 2: a tap shows on screen, nothing leaves the window.
// Hour 3 adds the POST /purchase call.

import { BRIDGE_URL } from "./config.js";
import { connectBridge } from "./bridge.js";

const gemCount = document.getElementById("gem-count");
const status = document.getElementById("status");
const link = document.getElementById("bridge-link");
const buttons = document.querySelectorAll(".buy");
const fakeTap = document.getElementById("fake-tap");

let gems = 0;
let selectedSku = null;

function setStatus(state, text) {
  status.dataset.state = state;
  status.textContent = text;
}

function setBuying(disabled) {
  for (const b of buttons) b.disabled = disabled;
}

function onPadEvent(event) {
  switch (event.event) {
    case "ready":
      setStatus("browsing", `Pad ready, firmware ${event.firmware}.`);
      break;
    case "tap":
      if (selectedSku) {
        setStatus("tap_detected", `Tap detected: card ${event.uid} for ${selectedSku}.`);
      } else {
        setStatus("browsing", `Tap detected: card ${event.uid}. Pick an item first.`);
      }
      break;
    case "error":
      setStatus("error", `Pad error: ${event.message}`);
      break;
  }
}

for (const button of buttons) {
  button.addEventListener("click", () => {
    selectedSku = button.closest(".item").dataset.sku;
    setBuying(true);
    setStatus("waiting_for_tap", `Tap your card on the pad to buy ${selectedSku}.`);
  });
}

connectBridge(BRIDGE_URL, {
  onEvent: onPadEvent,
  onLink(up) {
    link.dataset.up = String(up);
    link.textContent = up ? "pad connected" : "pad disconnected";
  },
});

// Dev tool, no hardware: injects a tap as if the bridge sent it.
fakeTap.addEventListener("click", () => onPadEvent({ event: "tap", uid: "04A3B2C1" }));

// Exposed for the next step, which grants gems once the server says paid.
window.tappad = {
  grant(amount) {
    gems += amount;
    gemCount.textContent = String(gems);
    selectedSku = null;
    setBuying(false);
    setStatus("browsing", `Added ${amount} gems.`);
  },
};
