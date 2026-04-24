import { describe, expect, test, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import {
  ResponseHeadersEditor,
  flatten,
  unflatten
} from "../ResponseHeadersEditor";
import type { GatewayRouteSummary } from "../../types";

const routes: GatewayRouteSummary[] = [
  {
    method: "GET",
    path: "/users",
    collection_name: "u",
    operation_id: null,
    summary: null,
    selected_example: null,
    available_examples: [],
    latency_ms: null
  }
];

describe("flatten / unflatten", () => {
  test("round-trips a two-level map", () => {
    const value = {
      "GET /users": { "x-request-id": "abc", "cache-control": "no-store" }
    };
    const flat = flatten(value);
    expect(flat).toHaveLength(2);
    expect(unflatten(flat)).toEqual(value);
  });

  test("skips rows with blank route or header name on unflatten", () => {
    expect(
      unflatten([
        { route: "GET /a", name: "x-ok", value: "1" },
        { route: "", name: "x-bad", value: "x" },
        { route: "GET /a", name: "", value: "x" }
      ])
    ).toEqual({ "GET /a": { "x-ok": "1" } });
  });

  test("later entries overwrite earlier ones for same (route, name)", () => {
    expect(
      unflatten([
        { route: "GET /a", name: "x", value: "first" },
        { route: "GET /a", name: "x", value: "second" }
      ])
    ).toEqual({ "GET /a": { x: "second" } });
  });
});

describe("ResponseHeadersEditor", () => {
  test("adds a new header and applies", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(
      <ResponseHeadersEditor
        running={true}
        routes={routes}
        value={{}}
        onApply={onApply}
      />
    );
    fireEvent.change(screen.getByLabelText("Route"), {
      target: { value: "GET /users" }
    });
    fireEvent.change(screen.getByLabelText("Header name"), {
      target: { value: "x-custom" }
    });
    fireEvent.change(screen.getByLabelText("Value"), {
      target: { value: "hello" }
    });
    fireEvent.click(
      screen.getByRole("button", { name: /Add \/ replace/i })
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^Apply/i }));
    });
    expect(onApply).toHaveBeenCalledWith({
      "GET /users": { "x-custom": "hello" }
    });
  });

  test("re-adding an existing (route, name) replaces the value", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(
      <ResponseHeadersEditor
        running={true}
        routes={routes}
        value={{ "GET /users": { "x-req-id": "old" } }}
        onApply={onApply}
      />
    );
    fireEvent.change(screen.getByLabelText("Route"), {
      target: { value: "GET /users" }
    });
    fireEvent.change(screen.getByLabelText("Header name"), {
      target: { value: "x-req-id" }
    });
    fireEvent.change(screen.getByLabelText("Value"), {
      target: { value: "new" }
    });
    fireEvent.click(
      screen.getByRole("button", { name: /Add \/ replace/i })
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^Apply/i }));
    });
    expect(onApply).toHaveBeenCalledWith({
      "GET /users": { "x-req-id": "new" }
    });
  });

  test("removes a row and applies the shrunk map", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(
      <ResponseHeadersEditor
        running={true}
        routes={routes}
        value={{
          "GET /users": { "x-a": "1", "x-b": "2" }
        }}
        onApply={onApply}
      />
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: /Remove x-a header for GET \/users/i
      })
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^Apply/i }));
    });
    expect(onApply).toHaveBeenCalledWith({
      "GET /users": { "x-b": "2" }
    });
  });
});
