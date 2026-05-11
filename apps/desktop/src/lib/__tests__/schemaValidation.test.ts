import { describe, expect, test } from "vitest";
import {
  pickResponseSchema,
  validateAgainstSchema
} from "../schemaValidation";
import type { CanonicalSchemaNode } from "../../types";

const objectSchema: CanonicalSchemaNode = {
  node_type: "object",
  properties: {
    id: { node_type: "string", required: true },
    active: { node_type: "boolean", required: false }
  }
};

describe("schemaValidation", () => {
  test("picks response schemas by example kind", () => {
    expect(
      pickResponseSchema(
        [
          { status_code: "200", schema: objectSchema },
          { status_code: "404", schema: { node_type: "object", properties: {} } }
        ],
        "success"
      )
    ).toBe(objectSchema);
  });

  test("reports required fields and type mismatches", () => {
    const errors = validateAgainstSchema(objectSchema, { active: "yes" });

    expect(errors).toContain("$.id: required property missing");
    expect(errors).toContain("$.active: expected boolean but got string");
  });

  test("walks array item schemas", () => {
    const errors = validateAgainstSchema(
      { node_type: "array", items: { node_type: "integer" } },
      [1, 2, "3"]
    );

    expect(errors).toContain("$[2]: expected integer but got string");
  });
});
