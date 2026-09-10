// The one error a game catches from this SDK. Mirrors SdkError in tappad-sdk.

/**
 * `kind` is one of:
 *  - "transport": the server could not be reached.
 *  - "server": non-2xx answer; `status` and `message` carry what it said.
 *  - "protocol": 2xx with a body that is not the documented shape.
 *  - "poll_timeout": the order never became final; `orderId` names it.
 */
export class TapPadError extends Error {
  /**
   * @param {"transport"|"server"|"protocol"|"poll_timeout"} kind
   * @param {string} message
   * @param {{status?: number, orderId?: number, cause?: unknown}} [extra]
   */
  constructor(kind, message, extra = {}) {
    super(message, extra.cause === undefined ? undefined : { cause: extra.cause });
    this.name = "TapPadError";
    this.kind = kind;
    if (extra.status !== undefined) this.status = extra.status;
    if (extra.orderId !== undefined) this.orderId = extra.orderId;
  }
}
