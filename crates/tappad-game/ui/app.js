// Wires DOM, bridge and server to the state machine in shop.js.
// This file owns side effects; shop.js owns decisions.

import { BRIDGE_URL, SERVER_URL } from "./config.js";
import { connectBridge } from "./bridge.js";
import { createServer } from "./server.js";
import { createFakeServer } from "./fake-server.js";
import { POLL_MS, initialState, transition } from "./shop.js";

const el = {
  gems: document.getElementById("gem-count"),
  status: document.getElementById("status"),
  link: document.getElementById("bridge-link"),
  buttons: document.querySelectorAll(".buy"),
  dismiss: document.getElementById("dismiss"),
  checkout: document.getElementById("checkout"),
  checkoutFrame: document.getElementById("checkout-frame"),
  checkoutCancel: document.getElementById("checkout-cancel"),
  fakeTapDad: document.getElementById("fake-tap-dad"),
  fakeTapKid: document.getElementById("fake-tap-kid"),
  fakeServer: document.getElementById("fake-server"),
};

const realServer = createServer(SERVER_URL);
const fakeServer = createFakeServer();
let state = initialState;
let pollTimer = null;

function server() {
  return el.fakeServer.checked ? fakeServer : realServer;
}

function dispatch(event) {
  const out = transition(state, event);
  state = out.state;
  render(state);
  for (const effect of out.effects) run(effect);
}

function run(effect) {
  switch (effect.type) {
    case "purchase":
      server()
        .purchase({ uid: effect.uid, sku: effect.sku })
        .then((response) => dispatch({ type: "response", response }))
        .catch((err) => dispatch({ type: "failure", message: err.message }));
      break;
    case "open_checkout":
      el.checkoutFrame.src = effect.url;
      el.checkout.hidden = false;
      break;
    case "close_checkout":
      el.checkout.hidden = true;
      el.checkoutFrame.src = "about:blank";
      break;
    case "poll":
      clearInterval(pollTimer);
      pollTimer = setInterval(() => {
        server()
          .orderStatus(effect.orderId)
          .then((status) => dispatch({ type: "order", state: status.state }))
          .catch((err) => dispatch({ type: "failure", message: err.message }));
      }, POLL_MS);
      break;
    case "stop_polling":
      clearInterval(pollTimer);
      pollTimer = null;
      break;
  }
}

function render(s) {
  el.gems.textContent = String(s.gems);
  el.status.dataset.state = s.ok === false ? "declined" : s.name;
  el.status.textContent = statusText(s);
  for (const b of el.buttons) b.disabled = s.name !== "browsing";
  el.dismiss.hidden = !(s.name === "result" || s.name === "waiting_for_tap");
  el.dismiss.textContent = s.name === "result" ? "Continue" : "Cancel";
}

function statusText(s) {
  switch (s.name) {
    case "browsing":
      return s.note;
    case "waiting_for_tap":
      return `Tap your card on the pad to buy ${s.sku}.`;
    case "purchasing":
      return `Card ${s.uid} tapped. Asking the server…`;
    case "awaiting_checkout":
      return `Order ${s.orderId}: finish the payment in the checkout.`;
    case "result":
      return s.text;
    default:
      return "";
  }
}

for (const button of el.buttons) {
  button.addEventListener("click", () => {
    dispatch({ type: "buy", sku: button.closest(".item").dataset.sku });
  });
}
el.dismiss.addEventListener("click", () => dispatch({ type: "dismiss" }));
el.checkoutCancel.addEventListener("click", () => dispatch({ type: "dismiss" }));

connectBridge(BRIDGE_URL, {
  onEvent(event) {
    if (event.event === "tap") dispatch({ type: "tap", uid: event.uid });
    else if (event.event === "error") el.status.textContent = `Pad error: ${event.message}`;
  },
  onLink(up) {
    el.link.dataset.up = String(up);
    el.link.textContent = up ? "pad connected" : "pad disconnected";
  },
});

// Dev tools, no hardware: inject a Dad or Kid tap as if the bridge sent it.
el.fakeTapDad.addEventListener("click", () => dispatch({ type: "tap", uid: "04A3B2C1" }));
el.fakeTapKid.addEventListener("click", () => dispatch({ type: "tap", uid: "0B1C2D3E" }));

render(state);
