// HTTP client for tappad-server. Two calls, shapes in docs/protocol.md.

/**
 * @param {string} baseUrl  http://127.0.0.1:8080
 * @returns {{ purchase(req: {uid: string, sku: string}): Promise<object>, orderStatus(orderId: number): Promise<object> }}
 */
export function createServer(baseUrl) {
  return {
    async purchase(req) {
      const res = await fetch(`${baseUrl}/purchase`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ uid: req.uid, sku: req.sku }),
      });
      return readJson(res);
    },
    async orderStatus(orderId) {
      const res = await fetch(`${baseUrl}/orders/${orderId}`);
      return readJson(res);
    },
  };
}

async function readJson(res) {
  if (!res.ok) throw new Error(`HTTP ${res.status}`);
  const body = await res.json();
  if (!body || typeof body !== "object") throw new Error("empty body");
  return body;
}
