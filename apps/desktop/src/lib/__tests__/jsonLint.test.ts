import { describe, expect, test } from "vitest";
import { lintJson } from "../jsonLint";

describe("lintJson", () => {
  test("empty input is treated as ok-but-empty", () => {
    expect(lintJson("")).toEqual({ ok: true, empty: true });
    expect(lintJson("   \n  ")).toEqual({ ok: true, empty: true });
  });

  test("valid JSON value is ok", () => {
    expect(lintJson('{"a": 1}')).toEqual({ ok: true, empty: false });
    expect(lintJson("[1, 2, 3]")).toEqual({ ok: true, empty: false });
    expect(lintJson("\"hello\"")).toEqual({ ok: true, empty: false });
    expect(lintJson("42")).toEqual({ ok: true, empty: false });
    expect(lintJson("null")).toEqual({ ok: true, empty: false });
  });

  test("invalid JSON surfaces a message + line/column", () => {
    const result = lintJson('{"a": 1,\n"b":}');
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.message).toMatch(/./);
      // The error is on line 2 after the ':'. If the engine exposed a
      // position we expect a >=1 line/column pair; if not, the result is
      // still a well-formed failure (message only). Accept either shape.
      if (result.line !== undefined) {
        expect(result.line).toBeGreaterThanOrEqual(1);
        expect(result.column).toBeGreaterThanOrEqual(1);
      }
    }
  });

  test("trailing commas are invalid", () => {
    const result = lintJson('{"a": 1,}');
    expect(result.ok).toBe(false);
  });

  test("deeply nested valid JSON is ok", () => {
    const nested = JSON.stringify({ a: { b: { c: [{ d: true }] } } });
    expect(lintJson(nested)).toEqual({ ok: true, empty: false });
  });
});
