// Dev-only stand-in for tappad-server so the page can be built before the server answers.
// Mirrors the mock provider's registry in PLAN.md: Gold pays, Starter is over the limit.
// Never used unless the "Fake server (dev)" box in the footer is ticked.

const GOLD = "04A3B2C1";
const STARTER = "04D4E5F6";
const SKUS = new Set(["gems_100", "gems_500", "gems_1200"]);

export function createFakeServer() {
  let nextOrder = 1;
  return {
    async purchase({ uid, sku }) {
      await delay(300);
      if (!SKUS.has(sku)) return { status: "declined", reason: "unknown_sku" };
      if (uid === STARTER) return { status: "declined", reason: "limit_exceeded" };
      if (uid !== GOLD) return { status: "declined", reason: "unknown_card" };
      const id = nextOrder++;
      return {
        status: "approved",
        order_id: id,
        receipt_id: `rcpt-${String(id).padStart(6, "0")}`,
      };
    },
    async orderStatus(orderId) {
      await delay(100);
      return { order_id: orderId, state: "paid" };
    },
  };
}

function delay(ms) {
  return new Promise((r) => setTimeout(r, ms));
}
