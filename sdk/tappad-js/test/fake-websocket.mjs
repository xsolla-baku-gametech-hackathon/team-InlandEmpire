// A WebSocket class driven by the test instead of the network.

export class FakeWebSocket {
  static instances = [];

  constructor(url) {
    this.url = url;
    this.listeners = { open: [], message: [], close: [], error: [] };
    this.closed = false;
    FakeWebSocket.instances.push(this);
  }

  addEventListener(type, fn) {
    this.listeners[type].push(fn);
  }

  /** Test helpers, not part of the WebSocket API. */
  emit(type, payload) {
    for (const fn of this.listeners[type]) fn(payload);
  }
  serverOpens() {
    this.emit("open", {});
  }
  serverSends(text) {
    this.emit("message", { data: text });
  }
  serverDrops() {
    this.closed = true;
    this.emit("close", {});
  }

  close() {
    if (this.closed) return;
    this.closed = true;
    this.emit("close", {});
  }
}
