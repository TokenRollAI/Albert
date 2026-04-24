/**
 * Map the raw error string surfaced by the `fetch_remote_source` Tauri
 * command into something we can show a user without exposing reqwest /
 * URL parser internals. The backend already prefixes each variant with
 * a category (`invalid URL:`, `fetch:`, `remote returned HTTP…`), so we
 * mostly match on the prefix and rewrite.
 */
export function friendlyFetchError(raw: unknown): string {
  const message = typeof raw === "string" ? raw : String(raw);
  const lower = message.toLowerCase();

  if (lower.startsWith("url is empty")) {
    return "Enter a URL to fetch.";
  }
  if (lower.startsWith("invalid url")) {
    return `That URL looks malformed. ${message.replace(/^invalid url:\s*/i, "")}`;
  }
  if (lower.startsWith("unsupported url scheme")) {
    return "Only http and https URLs are supported.";
  }
  if (lower.startsWith("fetch:")) {
    const inner = message.replace(/^fetch:\s*/i, "");
    if (inner.includes("tcp connect") || inner.includes("dns")) {
      return `Couldn't reach the server — check the URL or your connection (${inner}).`;
    }
    if (inner.includes("timeout")) {
      return "The request timed out after 20 s.";
    }
    return `Network error: ${inner}`;
  }
  if (lower.startsWith("remote returned http")) {
    return message;
  }
  if (lower.includes("exceeds")) {
    return `Response is too large to import (max 2 MB). ${message}`;
  }
  return message || "Fetch failed for an unknown reason.";
}

/**
 * Client-side URL sanity check run before we hit the Tauri command. This
 * avoids a round-trip for the obvious malformed cases and lets us
 * highlight the input while the user is still typing.
 */
export function validateFetchUrl(raw: string): string | null {
  const trimmed = raw.trim();
  if (trimmed.length === 0) return "Enter a URL.";
  try {
    const parsed = new URL(trimmed);
    if (parsed.protocol !== "http:" && parsed.protocol !== "https:") {
      return "Only http and https URLs are supported.";
    }
    if (!parsed.hostname) {
      return "URL is missing a hostname.";
    }
  } catch {
    return "That doesn't look like a valid URL.";
  }
  return null;
}
