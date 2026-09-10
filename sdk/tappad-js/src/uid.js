// Card UIDs, the same rules as `CardUid` in tappad-protocol.

/**
 * Normalises a UID the way the bridge and server do: separators out,
 * uppercase hex. Returns null when the result is not a 4, 7 or 10 byte UID.
 * @param {string} raw  "04:a3 b2-c1"
 * @returns {string|null}  "04A3B2C1"
 */
export function parseUid(raw) {
  if (typeof raw !== "string") return null;
  const digits = raw.replace(/[:\-\s]/g, "").toUpperCase();
  if (!/^[0-9A-F]*$/.test(digits)) return null;
  if (![8, 14, 20].includes(digits.length)) return null;
  return digits;
}
