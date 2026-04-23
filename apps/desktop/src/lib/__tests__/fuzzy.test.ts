import { describe, expect, test } from "vitest";
import { fuzzyFilter, fuzzyMatch } from "../fuzzy";

describe("fuzzyMatch", () => {
  test("empty query matches everything with zero score", () => {
    expect(fuzzyMatch("", "anything")).toEqual({ score: 0, indexes: [] });
  });

  test("returns null when a character is missing", () => {
    expect(fuzzyMatch("xyz", "hello world")).toBeNull();
  });

  test("matches out of order is rejected", () => {
    // 'ol' appears in "hello" but not before an 'e' that's prior — the
    // in-order requirement means "oeh" can't match "hello".
    expect(fuzzyMatch("oeh", "hello")).toBeNull();
  });

  test("word-boundary matches outscore mid-word ones", () => {
    // "gu" should match "GET /users" at the word boundary of /users
    // better than "getusers-manage" where 'u' is mid-word.
    const boundary = fuzzyMatch("gu", "GET /users");
    const midword = fuzzyMatch("gu", "zagury");
    expect(boundary).not.toBeNull();
    expect(midword).not.toBeNull();
    expect(boundary!.score).toBeGreaterThan(midword!.score);
  });

  test("contiguous matches rank higher than gapped ones", () => {
    const contiguous = fuzzyMatch("user", "users list")!;
    const gapped = fuzzyMatch("user", "upload server")!;
    expect(contiguous.score).toBeGreaterThan(gapped.score);
  });

  test("indexes mark every matched character", () => {
    const match = fuzzyMatch("hw", "hello world")!;
    expect(match.indexes).toEqual([0, 6]);
  });

  test("is case-insensitive", () => {
    expect(fuzzyMatch("GET", "get /users")).not.toBeNull();
    expect(fuzzyMatch("users", "USERS")).not.toBeNull();
  });
});

describe("fuzzyFilter", () => {
  test("drops non-matches and sorts by score", () => {
    const items = [
      "GET /users",
      "POST /orders",
      "GET /accounts/{id}",
      "DELETE /accounts/{id}"
    ];
    const result = fuzzyFilter("users", items, (s) => s);
    expect(result).toHaveLength(1);
    expect(result[0].item).toBe("GET /users");
  });

  test("keeps input order on ties", () => {
    const items = ["alpha foo", "beta foo"];
    const result = fuzzyFilter("foo", items, (s) => s);
    expect(result.map((r) => r.item)).toEqual(["alpha foo", "beta foo"]);
  });

  test("empty query returns everything in original order", () => {
    const items = ["a", "b", "c"];
    expect(fuzzyFilter("", items, (s) => s).map((r) => r.item)).toEqual([
      "a",
      "b",
      "c"
    ]);
  });
});
