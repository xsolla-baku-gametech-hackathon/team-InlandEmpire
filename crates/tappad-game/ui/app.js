// Shop page state. Hour 1: browsing and waiting for a tap, nothing leaves the window.
// Hour 2 adds the WebSocket tap listener, hour 3 the POST /purchase call.

const gemCount = document.getElementById("gem-count");
const status = document.getElementById("status");
const buttons = document.querySelectorAll(".buy");

let gems = 0;

function setStatus(state, text) {
  status.dataset.state = state;
  status.textContent = text;
}

function setBuying(disabled) {
  for (const b of buttons) b.disabled = disabled;
}

for (const button of buttons) {
  button.addEventListener("click", () => {
    const item = button.closest(".item");
    const sku = item.dataset.sku;
    setBuying(true);
    setStatus("waiting_for_tap", `Tap your card on the pad to buy ${sku}.`);
  });
}

// Exposed for the next step, which grants gems once the server says paid.
window.tappad = {
  grant(amount) {
    gems += amount;
    gemCount.textContent = String(gems);
    setBuying(false);
    setStatus("browsing", `Added ${amount} gems.`);
  },
};
