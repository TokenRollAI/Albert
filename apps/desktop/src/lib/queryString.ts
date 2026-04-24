/**
 * URL-encoded query-string ⇄ key/value pair round-trip helpers used by
 * the Try-it panel's structured query builder. Deliberately permissive:
 * we pass through repeated keys (they're legal in most HTTP APIs) and
 * empty values ("foo=" keeps the key) so users can round-trip whatever
 * their API actually sends.
 */
export interface QueryPair {
  key: string;
  value: string;
}

/**
 * Parse a raw query string (with or without a leading `?`) into an
 * ordered list of key=value pairs. Gracefully handles standalone keys
 * (`?a&b=2`) and empty values (`?foo=`).
 */
export function parseQueryString(raw: string): QueryPair[] {
  const trimmed = raw.trim().replace(/^\?/, "");
  if (!trimmed) return [];
  return trimmed.split("&").map((pair) => {
    const eq = pair.indexOf("=");
    if (eq === -1) {
      return { key: safeDecode(pair), value: "" };
    }
    return {
      key: safeDecode(pair.slice(0, eq)),
      value: safeDecode(pair.slice(eq + 1))
    };
  });
}

/**
 * Serialize rows back into a `key=value&...` form. Rows whose key is
 * blank (after trim) are dropped — they represent in-progress edits the
 * user hasn't committed.
 */
export function serializeQueryString(pairs: QueryPair[]): string {
  return pairs
    .filter((p) => p.key.trim().length > 0)
    .map((p) => `${encodeURIComponent(p.key)}=${encodeURIComponent(p.value)}`)
    .join("&");
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value.replace(/\+/g, " "));
  } catch {
    return value;
  }
}
