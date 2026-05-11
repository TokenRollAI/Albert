import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import {
  ConditionalRulesEditor,
  normalizeRules
} from "../ConditionalRulesEditor";
import type {
  ConditionalExampleRule,
  GatewayRouteSummary
} from "../../types";

const routes: GatewayRouteSummary[] = [
  {
    method: "GET",
    path: "/orders",
    collection_name: "Orders",
    operation_id: "listOrders",
    summary: "List orders",
    selected_example: "success",
    available_examples: ["success", "empty", "error"],
    latency_ms: null
  }
];

describe("ConditionalRulesEditor", () => {
  test("adds a query rule and applies gateway-shaped config", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(
      <ConditionalRulesEditor
        running={true}
        routes={routes}
        value={{}}
        onApply={onApply}
      />
    );

    fireEvent.change(screen.getByLabelText("Rule name"), {
      target: { value: "VIP empty list" }
    });
    fireEvent.change(screen.getByLabelText("Example"), {
      target: { value: "empty" }
    });
    fireEvent.click(screen.getByRole("button", { name: /Add rule/i }));
    fireEvent.click(screen.getByRole("button", { name: /Apply rules/i }));

    await waitFor(() =>
      expect(onApply).toHaveBeenCalledWith({
        "GET /orders": [
          {
            name: "VIP empty list",
            example: "empty",
            when: [{ source: "query", name: "status", equals: "vip" }]
          }
        ]
      })
    );
  });

  test("serializes body condition values as JSON when possible", async () => {
    const onApply = vi.fn().mockResolvedValue(undefined);
    render(
      <ConditionalRulesEditor
        running={true}
        routes={routes}
        value={{}}
        onApply={onApply}
      />
    );

    fireEvent.change(screen.getByLabelText("Condition"), {
      target: { value: "body" }
    });
    fireEvent.change(screen.getByLabelText("Body path"), {
      target: { value: "items.0.qty" }
    });
    fireEvent.change(screen.getByLabelText("Equals"), {
      target: { value: "2" }
    });
    fireEvent.click(screen.getByRole("button", { name: /Add rule/i }));
    fireEvent.click(screen.getByRole("button", { name: /Apply rules/i }));

    await waitFor(() => {
      const payload = onApply.mock.calls[0]?.[0] as Record<
        string,
        ConditionalExampleRule[]
      >;
      expect(payload["GET /orders"][0].when[0]).toEqual({
        source: "body",
        path: "items.0.qty",
        equals: 2
      });
    });
  });

  test("normalizes away empty conditions and routes", () => {
    expect(
      normalizeRules({
        " GET /orders ": [
          {
            name: " ",
            example: "success",
            when: [
              { source: "query", name: " status ", equals: "empty" },
              { source: "header", name: " ", equals: "x" }
            ]
          }
        ],
        " ": [
          {
            name: "dropped",
            example: "error",
            when: [{ source: "query", name: "x", equals: "y" }]
          }
        ]
      })
    ).toEqual({
      "GET /orders": [
        {
          name: "Conditional rule",
          example: "success",
          when: [{ source: "query", name: "status", equals: "empty" }]
        }
      ]
    });
  });
});
