import { describe, expect, test, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { StatusOverridesEditor } from "../StatusOverridesEditor";
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
  },
  {
    method: "POST",
    path: "/orders",
    collection_name: "o",
    operation_id: null,
    summary: null,
    selected_example: null,
    available_examples: [],
    latency_ms: null
  }
];

describe("StatusOverridesEditor", () => {
  test("adds a new override and ships it via onApply", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(
      <StatusOverridesEditor
        running={true}
        routes={routes}
        value={{}}
        onApply={onApply}
      />
    );
    fireEvent.change(screen.getByLabelText("Route"), {
      target: { value: "POST /orders" }
    });
    fireEvent.change(screen.getByLabelText(/Status/), {
      target: { value: "201" }
    });
    fireEvent.click(
      screen.getByRole("button", { name: /Add \/ replace/i })
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^Apply/i }));
    });
    expect(onApply).toHaveBeenCalledWith({ "POST /orders": 201 });
  });

  test("rejects out-of-range codes at the UI layer", () => {
    render(
      <StatusOverridesEditor
        running={true}
        routes={routes}
        value={{}}
        onApply={vi.fn()}
      />
    );
    fireEvent.change(screen.getByLabelText(/Status/), {
      target: { value: "999" }
    });
    // Add button is disabled when the code is out of range.
    const addBtn = screen.getByRole("button", {
      name: /Add \/ replace/i
    }) as HTMLButtonElement;
    expect(addBtn.disabled).toBe(true);
    expect(screen.getByText(/status must be between/i)).toBeTruthy();
  });

  test("removes an existing override and applies the shrunk set", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(
      <StatusOverridesEditor
        running={true}
        routes={routes}
        value={{ "GET /users": 418, "POST /orders": 201 }}
        onApply={onApply}
      />
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: /Remove status override for GET \/users/i
      })
    );
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^Apply/i }));
    });
    expect(onApply).toHaveBeenCalledWith({ "POST /orders": 201 });
  });
});
