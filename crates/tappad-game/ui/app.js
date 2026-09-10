// Wires DOM, bridge and server to the state machine in shop.js.
// This file owns side effects; shop.js owns decisions.

import { BRIDGE_URL, SERVER_URL } from "./config.js";
import { connectBridge } from "./bridge.js";
import { createServer } from "./server.js";
import { createFakeServer } from "./fake-server.js";
import { POLL_MS, gemsFor, initialState, transition } from "./shop.js";

const el = {
  gems: document.getElementById("gem-count"),
  wallet: document.querySelector(".wallet"),
  status: document.getElementById("status"),
  link: document.getElementById("bridge-link"),
  shop: document.getElementById("shop"),
  sheet: document.getElementById("sheet"),
  sheetBox: document.querySelector(".sheet-box"),
  sheetTitle: document.getElementById("sheet-title"),
  sheetText: document.getElementById("sheet-text"),
  sheetDelta: document.getElementById("sheet-delta"),
  dismiss: document.getElementById("dismiss"),
  checkout: document.getElementById("checkout"),
  checkoutFrame: document.getElementById("checkout-frame"),
  checkoutCancel: document.getElementById("checkout-cancel"),
  fakeTapGold: document.getElementById("fake-tap-gold"),
  fakeTapStarter: document.getElementById("fake-tap-starter"),
  fakeServer: document.getElementById("fake-server"),
};

const realServer = createServer(SERVER_URL);
const fakeServer = createFakeServer();
let state = initialState;
let pollTimer = null;
let renderedItems = null;

function server() {
  return el.fakeServer.checked ? fakeServer : realServer;
}

function dispatch(event) {
  const before = state;
  const out = transition(state, event);
  state = out.state;
  render(state, before);
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

function render(s, before) {
  if (s.items !== renderedItems) renderShop(s.items);
  el.gems.textContent = String(s.gems);
  if (before && s.gems > before.gems) {
    el.wallet.classList.remove("bump");
    void el.wallet.offsetWidth; // restart the animation
    el.wallet.classList.add("bump");
  }
  el.status.dataset.state = s.ok === false ? "declined" : s.name;
  el.status.textContent = statusText(s);
  for (const b of el.shop.querySelectorAll(".buy")) b.disabled = s.name !== "browsing";
  renderSheet(s, before);
}

function renderShop(items) {
  renderedItems = items;
  const featured = items.length >= 3 ? items[Math.floor(items.length / 2)].sku : null;
  el.shop.replaceChildren(...items.map((item) => itemCard(item, item.sku === featured)));
}

function itemCard(item, featured) {
  const card = document.createElement("article");
  card.className = featured ? "item featured" : "item";
  card.dataset.sku = item.sku;
  if (featured) card.appendChild(text("span", "badge", "Most popular"));
  const tier = document.createElement("div");
  tier.className = "tier";
  tier.appendChild(item.image_url ? image(item) : gemIcon(gemsFor(item)));
  card.appendChild(tier);
  card.appendChild(text("h2", "", item.name));
  card.appendChild(text("p", "desc", item.description === item.name ? "" : item.description));
  card.appendChild(text("p", "price", money(item)));
  const buy = document.createElement("button");
  buy.type = "button";
  buy.className = "buy";
  buy.textContent = "Buy";
  card.appendChild(buy);
  return card;
}

function text(tag, className, content) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  node.textContent = content;
  return node;
}

function image(item) {
  const img = document.createElement("img");
  img.src = item.image_url;
  img.alt = "";
  img.width = 64;
  img.height = 64;
  return img;
}

// Bigger packs get more gems in the icon: 1 up to 199, 2 up to 999, 3 above.
function gemIcon(gems) {
  const count = gems >= 1000 ? 3 : gems >= 200 ? 2 : 1;
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 64 64");
  svg.setAttribute("width", "56");
  svg.setAttribute("height", "56");
  const spots = [[32, 34, 1], [20, 22, 0.8], [46, 20, 0.7]].slice(0, count);
  for (const [x, y, k] of spots) {
    const p = document.createElementNS("http://www.w3.org/2000/svg", "path");
    p.setAttribute("d", "M-12 -10h24l8 12-20 24-20-24z");
    p.setAttribute("transform", `translate(${x} ${y}) scale(${k})`);
    p.setAttribute("fill", "url(#g)");
    svg.appendChild(p);
  }
  return svg;
}

function money(item) {
  const amount = (item.price / 100).toFixed(2);
  return item.currency === "USD" ? `$${amount}` : `${amount} ${item.currency}`;
}

function statusText(s) {
  switch (s.name) {
    case "browsing":
      return s.note;
    case "waiting_for_tap":
      return `Tap your card on the pad to buy ${s.items.find((i) => i.sku === s.sku)?.name ?? s.sku}.`;
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

function renderSheet(s, before) {
  const show = s.name === "waiting_for_tap" || s.name === "purchasing" || s.name === "result";
  el.sheet.hidden = !show;
  if (!show) return;
  const item = s.items.find((i) => i.sku === s.sku);
  switch (s.name) {
    case "waiting_for_tap":
      el.sheetBox.dataset.state = "waiting_for_tap";
      el.sheetTitle.textContent = "Tap your card";
      el.sheetText.textContent = `${item?.name ?? s.sku} for ${item ? money(item) : ""}. Hold your card on the pad.`;
      break;
    case "purchasing":
      el.sheetBox.dataset.state = "purchasing";
      el.sheetTitle.textContent = "Charging your card";
      el.sheetText.textContent = `Card ${s.uid}. One moment.`;
      break;
    case "result":
      el.sheetBox.dataset.state = s.ok ? "ok" : "declined";
      el.sheetTitle.textContent = s.ok ? "Paid" : "Not this time";
      el.sheetText.textContent = s.text;
      break;
  }
  const delta = before && s.name === "result" ? s.gems - before.gems : 0;
  el.sheetDelta.hidden = delta <= 0;
  el.sheetDelta.textContent = delta > 0 ? `+${delta} gems` : "";
  el.dismiss.hidden = s.name === "purchasing";
  el.dismiss.textContent = s.name === "result" ? "Continue" : "Cancel";
}

el.shop.addEventListener("click", (e) => {
  const buy = e.target.closest(".buy");
  if (buy) dispatch({ type: "buy", sku: buy.closest(".item").dataset.sku });
});
el.dismiss.addEventListener("click", () => dispatch({ type: "dismiss" }));
el.checkoutCancel.addEventListener("click", () => dispatch({ type: "dismiss" }));

connectBridge(BRIDGE_URL, {
  onEvent(event) {
    if (event.event === "tap") dispatch({ type: "tap", uid: event.uid });
    else if (event.event === "error") el.status.textContent = `Pad error: ${event.message}`;
  },
  onLink(up) {
    el.link.dataset.up = String(up);
    el.link.lastElementChild.textContent = up ? "pad connected" : "pad offline";
  },
});

// Dev tools, no hardware: inject a Gold or Starter tap as if the bridge sent it.
el.fakeTapGold.addEventListener("click", () => dispatch({ type: "tap", uid: "04A3B2C1" }));
el.fakeTapStarter.addEventListener("click", () => dispatch({ type: "tap", uid: "04D4E5F6" }));

render(state);
server()
  .catalog()
  .then((items) => dispatch({ type: "catalog", items }))
  .catch(() => {}); // keep DEFAULT_ITEMS when the server is down
