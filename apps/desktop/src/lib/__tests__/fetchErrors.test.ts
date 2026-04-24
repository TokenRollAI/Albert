import { describe, expect, test } from "vitest";
import { friendlyFetchError, validateFetchUrl } from "../fetchErrors";

describe("validateFetchUrl", () => {
  test("blank input asks the user to enter something", () => {
    expect(validateFetchUrl("")).toMatch(/enter/i);
    expect(validateFetchUrl("   ")).toMatch(/enter/i);
  });

  test("non-http(s) schemes are rejected up front", () => {
    expect(validateFetchUrl("file:///tmp/x")).toMatch(/http and https/);
    expect(validateFetchUrl("ftp://example.com")).toMatch(/http and https/);
  });

  test("unparsable strings return a diagnostic", () => {
    expect(validateFetchUrl("not a url")).toMatch(/valid URL/i);
  });

  test("valid https URL returns null", () => {
    expect(validateFetchUrl("https://api.example.com/openapi.json")).toBe(
      null
    );
  });

  test("strictly malformed URLs are rejected", () => {
    // Missing scheme — URL constructor should throw.
    expect(validateFetchUrl("example.com/path")).not.toBe(null);
  });
});

describe("friendlyFetchError", () => {
  test("handles empty-URL variant", () => {
    expect(friendlyFetchError("URL is empty")).toMatch(/enter a URL/i);
  });

  test("rewrites invalid URL errors with the original detail", () => {
    const msg = friendlyFetchError("invalid URL: relative URL without a base");
    expect(msg).toMatch(/malformed/i);
    expect(msg).toMatch(/without a base/);
  });

  test("unsupported scheme gets a clear message", () => {
    expect(friendlyFetchError("unsupported URL scheme: ftp")).toMatch(
      /http and https/
    );
  });

  test("network errors carry through the inner detail", () => {
    const msg = friendlyFetchError(
      "fetch: error sending request for url (tcp connect error)"
    );
    expect(msg).toMatch(/couldn't reach/i);
  });

  test("timeout errors are labeled explicitly", () => {
    const msg = friendlyFetchError("fetch: operation timeout");
    expect(msg).toMatch(/timed out/i);
  });

  test("HTTP 4xx / 5xx responses pass through verbatim", () => {
    const msg = friendlyFetchError(
      "remote returned HTTP 404 Not Found: body snippet"
    );
    expect(msg).toMatch(/remote returned HTTP 404/);
  });

  test("oversized payload errors are prefixed helpfully", () => {
    const msg = friendlyFetchError(
      "response exceeds 2097152 bytes (3145728 bytes)"
    );
    expect(msg).toMatch(/too large/i);
    expect(msg).toMatch(/2 MB/);
  });

  test("unknown errors fall back to a generic phrase", () => {
    expect(friendlyFetchError("")).toMatch(/unknown/i);
  });
});
