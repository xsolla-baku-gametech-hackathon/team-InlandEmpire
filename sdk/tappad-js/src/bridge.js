// Taps from tappad-bridge over WebSocket. Reconnects on its own: the bridge may
// start after the game, or the pad may be unplugged and plugged back in.

import { parseUid } from "./uid.js";

/**
 * One JSON object per frame, shapes in docs/protocol.md. Anything else is
 * null: chip boot noise, unknown events and bad uids all land there.
 * @param {string} text
 * @returns {{event: "ready", firmware: string} | {event: "tap", uid: string} | {event: "error", message: string} | null}
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
      return typeof obj.firmware === "string" ? { event: "ready", firmware: obj.firmware } : null;
    case "tap": {
      const uid = parseUid(obj.uid);
      return uid ? { event: "tap", uid } : null;
    }
    case "error":
      return typeof obj.message === "string" ? { event: "error", message: obj.message } : null;
    default:
      return null;
  }
}

/**
 * Connects to the bridge and keeps connecting until `stop()` is called.
 * @param {string} url  "ws://127.0.0.1:8765"
 * @param {{onEvent: (event: object) => void, onLink?: (up: boolean) => void}} handlers
 * @param {{WebSocket?: typeof WebSocket, retryMs?: number}} [deps]
 *   a WebSocket class to use instead of the global one, and the reconnect gap (2000 ms).
 * @returns {{stop: () => void}}
 */
export function connectBridge(url, handlers, deps = {}) {
  const WS = "WebSocket" in deps ? deps.WebSocket : globalThis.WebSocket;
  if (typeof WS !== "function") {
    throw new TypeError("no WebSocket available; pass one in deps.WebSocket");
  }
  const retryMs = deps.retryMs ?? 2000;
  const onLink = handlers.onLink ?? (() => {});
  let socket = null;
  let timer = null;
  let stopped = false;

  function open() {
    socket = new WS(url);
    socket.addEventListener("open", () => onLink(true));
    socket.addEventListener("message", (msg) => {
      const event = parsePadEvent(typeof msg.data === "string" ? msg.data : "");
      if (event) handlers.onEvent(event);
    });
    socket.addEventListener("close", () => {
      onLink(false);
      if (!stopped) timer = setTimeout(open, retryMs);
    });
    socket.addEventListener("error", () => socket.close());
  }

  open();
  return {
    stop() {
      stopped = true;
      clearTimeout(timer);
      if (socket) socket.close();
    },
  };
}
