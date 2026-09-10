// Listens to the bridge and hands every pad event to one callback.
// Reconnects on its own: the bridge may start after the game, or be unplugged.

const RETRY_MS = 2000;

/**
 * Connects to the bridge and keeps connecting.
 * @param {string} url  ws:// address of tappad-bridge
 * @param {{ onEvent: (event: object) => void, onLink: (up: boolean) => void }} handlers
 * @returns {() => void} stop function
 */
export function connectBridge(url, handlers) {
  let socket = null;
  let timer = null;
  let stopped = false;

  function open() {
    socket = new WebSocket(url);
    socket.addEventListener("open", () => handlers.onLink(true));
    socket.addEventListener("message", (msg) => {
      const event = parsePadEvent(msg.data);
      if (event) handlers.onEvent(event);
    });
    socket.addEventListener("close", () => {
      handlers.onLink(false);
      if (!stopped) timer = setTimeout(open, RETRY_MS);
    });
    socket.addEventListener("error", () => socket.close());
  }

  open();
  return () => {
    stopped = true;
    clearTimeout(timer);
    if (socket) socket.close();
  };
}

/**
 * One JSON object per frame, shape in docs/protocol.md. Anything else is dropped.
 * @param {string} text
 * @returns {object|null}
 */
export function parsePadEvent(text) {
  let obj;
  try {
    obj = JSON.parse(text);
  } catch {
    return null;
  }
  if (!obj || typeof obj.event !== "string") return null;
  switch (obj.event) {
    case "ready":
      return typeof obj.firmware === "string" ? obj : null;
    case "tap":
      return typeof obj.uid === "string" ? { event: "tap", uid: normaliseUid(obj.uid) } : null;
    case "error":
      return typeof obj.message === "string" ? obj : null;
    default:
      return null;
  }
}

/** `04:a3 b2c1` becomes `04A3B2C1`, the same rule the bridge and server apply. */
export function normaliseUid(raw) {
  return raw.replace(/[:\s]/g, "").toUpperCase();
}
