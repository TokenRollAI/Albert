import { describe, expect, test } from "vitest";
import { parseQueryString, serializeQueryString } from "../queryString";

describe("parseQueryString", () => {
  test("empty input yields empty array", () => {
    expect(parseQueryString("")).toEqual([]);
    expect(parseQueryString("   ")).toEqual([]);
    expect(parseQueryString("?")).toEqual([]);
  });

  test("key=value pairs preserve order", () => {
    expect(parseQueryString("a=1&b=2")).toEqual([
      { key: "a", value: "1" },
      { key: "b", value: "2" }
    ]);
  });

  test("strips leading ?", () => {
    expect(parseQueryString("?status=paid")).toEqual([
      { key: "status", value: "paid" }
    ]);
  });

  test("empty value keeps the key", () => {
    expect(parseQueryString("foo=")).toEqual([{ key: "foo", value: "" }]);
  });

  test("standalone key has empty value", () => {
    expect(parseQueryString("a&b=2")).toEqual([
      { key: "a", value: "" },
      { key: "b", value: "2" }
    ]);
  });

  test("percent-decodes both keys and values", () => {
    expect(parseQueryString("tag=hello%20world")).toEqual([
      { key: "tag", value: "hello world" }
    ]);
    expect(parseQueryString("q=a%2Bb")).toEqual([
      { key: "q", value: "a+b" }
    ]);
  });

  test("+ is treated as space", () => {
    expect(parseQueryString("q=hello+world")).toEqual([
      { key: "q", value: "hello world" }
    ]);
  });

  test("malformed % escapes are passed through untouched", () => {
    expect(parseQueryString("q=%ZZ")).toEqual([{ key: "q", value: "%ZZ" }]);
  });
});

describe("serializeQueryString", () => {
  test("round-trips simple cases", () => {
    const raw = "a=1&b=2";
    expect(serializeQueryString(parseQueryString(raw))).toBe(raw);
  });

  test("encodes spaces and special chars", () => {
    expect(
      serializeQueryString([
        { key: "q", value: "hello world" },
        { key: "tag", value: "a+b" }
      ])
    ).toBe("q=hello%20world&tag=a%2Bb");
  });

  test("drops rows with blank keys", () => {
    expect(
      serializeQueryString([
        { key: "a", value: "1" },
        { key: "   ", value: "orphan" },
        { key: "b", value: "" }
      ])
    ).toBe("a=1&b=");
  });

  test("empty input serializes to empty string", () => {
    expect(serializeQueryString([])).toBe("");
  });
});
