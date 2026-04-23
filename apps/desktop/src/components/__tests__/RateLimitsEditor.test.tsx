import { describe, expect, test, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { RateLimitsEditor } from "../RateLimitsEditor";
import type { GatewayRouteSummary, RateLimitRule } from "../../types";

const routes: GatewayRouteSummary[] = [
  {
    method: "GET",
    path: "/users",
    collection_name: "users",
    operation_id: null,
    summary: null,
    selected_example: null,
    available_examples: [],
    latency_ms: null
  },
  {
    method: "POST",
    path: "/orders",
    collection_name: "orders",
    operation_id: null,
    summary: null,
    selected_example: null,
    available_examples: [],
    latency_ms: null
  }
];

describe("RateLimitsEditor", () => {
  test("adds a new rule and fires Apply with the full draft", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(
      <RateLimitsEditor
        running={true}
        routes={routes}
        value={{}}
        onApply={onApply}
      />
    );

    fireEvent.change(screen.getByLabelText("Route"), {
      target: { value: "POST /orders" }
    });
    fireEvent.change(screen.getByLabelText("Limit"), {
      target: { value: "5" }
    });
    fireEvent.change(screen.getByLabelText("Window (ms)"), {
      target: { value: "2000" }
    });
    fireEvent.click(
      screen.getByRole("button", { name: /Add \/ replace rule/i })
    );

    const apply = screen.getByRole("button", {
      name: /^Apply/i
    }) as HTMLButtonElement;
    expect(apply.disabled).toBe(false);
    await act(async () => {
      fireEvent.click(apply);
    });
    expect(onApply).toHaveBeenCalledTimes(1);
    expect(onApply).toHaveBeenCalledWith({
      "POST /orders": { limit: 5, window_ms: 2000 }
    });
  });

  test("removes a rule and applies the shrunk set", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    const initial: Record<string, RateLimitRule> = {
      "GET /users": { limit: 10, window_ms: 1000 },
      "POST /orders": { limit: 3, window_ms: 500 }
    };
    render(
      <RateLimitsEditor
        running={true}
        routes={routes}
        value={initial}
        onApply={onApply}
      />
    );

    fireEvent.click(
      screen.getByRole("button", {
        name: /Remove rate limit for GET \/users/i
      })
    );

    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /^Apply/i }));
    });
    expect(onApply).toHaveBeenCalledWith({
      "POST /orders": { limit: 3, window_ms: 500 }
    });
  });

  test("Apply disabled when the draft equals the current value", () => {
    const initial: Record<string, RateLimitRule> = {
      "GET /users": { limit: 10, window_ms: 1000 }
    };
    render(
      <RateLimitsEditor
        running={true}
        routes={routes}
        value={initial}
        onApply={vi.fn()}
      />
    );
    const apply = screen.getByRole("button", {
      name: /^Apply/i
    }) as HTMLButtonElement;
    expect(apply.disabled).toBe(true);
  });
});
