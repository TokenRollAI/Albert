import { describe, expect, test } from "vitest";
import { render } from "@testing-library/react";
import { JsonView } from "../JsonView";

describe("JsonView", () => {
  test("renders keys with the key token class", () => {
    const { container } = render(<JsonView value={{ hello: "world" }} />);
    const keys = container.querySelectorAll(".jsont--key");
    expect(keys).toHaveLength(1);
    expect(keys[0]?.textContent).toContain("hello");
  });

  test("renders strings, numbers, booleans, and null with their token classes", () => {
    const { container } = render(
      <JsonView
        value={{ name: "Ada", age: 42, active: true, pet: null }}
      />
    );
    expect(container.querySelectorAll(".jsont--string")).toHaveLength(1);
    expect(container.querySelectorAll(".jsont--number")).toHaveLength(1);
    expect(container.querySelectorAll(".jsont--boolean")).toHaveLength(1);
    expect(container.querySelectorAll(".jsont--null")).toHaveLength(1);
  });

  test("handles arrays and nested objects", () => {
    const { container } = render(
      <JsonView value={{ tags: ["one", "two"], nested: { ok: false } }} />
    );
    // bracket tokens for the array, brace tokens for the nested object
    expect(container.querySelectorAll(".jsont--bracket").length).toBeGreaterThanOrEqual(2);
    expect(container.querySelectorAll(".jsont--brace").length).toBeGreaterThanOrEqual(4);
  });

  test("falls back gracefully on serialization errors", () => {
    // Circular reference — JSON.stringify throws, JsonView should still
    // render something rather than tearing down.
    const circular: Record<string, unknown> = { name: "self" };
    circular.self = circular;
    const { container } = render(<JsonView value={circular} />);
    expect(container.querySelector("pre")).not.toBeNull();
  });
});
