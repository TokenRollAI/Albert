import { describe, expect, test } from "vitest";
import { importChangeGenerationContext } from "../importReportContext";

describe("importChangeGenerationContext", () => {
  test("turns changed endpoint reasons into prompt context", () => {
    expect(
      importChangeGenerationContext({
        method: "get",
        path: "/orders/{id}",
        summary: "Fetch order",
        reasons: ["parameters changed", "responses changed"],
        details: ["parameter added: query status", "response changed: 200 (schema)"]
      })
    ).toEqual({
      note: "Re-import detected endpoint contract drift for GET /orders/{id}: parameters changed, responses changed. Details: parameter added: query status; response changed: 200 (schema). Refresh the success mock so it stays aligned with the changed API contract."
    });
  });

  test("returns no context when the import diff has no reasons", () => {
    expect(
      importChangeGenerationContext({
        method: "POST",
        path: "/orders",
        summary: "Create order"
      })
    ).toBeNull();
  });
});
