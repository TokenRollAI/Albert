import type { CanonicalSchemaNode, ExampleKind } from "../types";

export function pickResponseSchema(
  responses: Array<{ status_code: string; schema?: CanonicalSchemaNode | null }>,
  kind: ExampleKind
): CanonicalSchemaNode | null {
  const match =
    responses.find((response) => matchesKind(kind, response.status_code)) ??
    null;
  return match?.schema ?? null;
}

export function validateAgainstSchema(
  schema: CanonicalSchemaNode | null | undefined,
  value: unknown
): string[] {
  if (!schema) return [];
  const errors: string[] = [];
  validateNode(schema, value, "$", errors);
  return errors;
}

function matchesKind(kind: ExampleKind, status: string): boolean {
  if (kind === "success") return status.startsWith("2") || status.startsWith("3");
  if (kind === "empty") return status === "204";
  return status.startsWith("4") || status.startsWith("5");
}

function validateNode(
  schema: CanonicalSchemaNode,
  value: unknown,
  path: string,
  errors: string[]
): void {
  if (value === null) {
    if (schema.nullable || schema.node_type === "null" || schema.node_type === "unknown") {
      return;
    }
    errors.push(`${path}: expected ${schema.node_type} but got null`);
    return;
  }
  if (schema.enum_values?.length && !schema.enum_values.some((item) => deepEqual(item, value))) {
    errors.push(`${path}: value is not in the declared enum`);
    return;
  }

  switch (schema.node_type) {
    case "object":
      validateObject(schema, value, path, errors);
      break;
    case "array":
      validateArray(schema, value, path, errors);
      break;
    case "string":
      if (typeof value !== "string") errors.push(`${path}: expected string but got ${jsonType(value)}`);
      break;
    case "integer":
      if (typeof value !== "number" || !Number.isInteger(value)) {
        errors.push(`${path}: expected integer but got ${jsonType(value)}`);
      }
      break;
    case "number":
      if (typeof value !== "number") errors.push(`${path}: expected number but got ${jsonType(value)}`);
      break;
    case "boolean":
      if (typeof value !== "boolean") errors.push(`${path}: expected boolean but got ${jsonType(value)}`);
      break;
    case "null":
      errors.push(`${path}: expected null but got ${jsonType(value)}`);
      break;
    case "unknown":
    default:
      break;
  }
}

function validateObject(
  schema: CanonicalSchemaNode,
  value: unknown,
  path: string,
  errors: string[]
): void {
  if (!isRecord(value)) {
    errors.push(`${path}: expected object but got ${jsonType(value)}`);
    return;
  }
  for (const [name, child] of Object.entries(schema.properties ?? {})) {
    if (Object.prototype.hasOwnProperty.call(value, name)) {
      validateNode(child, value[name], `${path}.${name}`, errors);
    } else if (child.required) {
      errors.push(`${path}.${name}: required property missing`);
    }
  }
}

function validateArray(
  schema: CanonicalSchemaNode,
  value: unknown,
  path: string,
  errors: string[]
): void {
  if (!Array.isArray(value)) {
    errors.push(`${path}: expected array but got ${jsonType(value)}`);
    return;
  }
  if (!schema.items) return;
  value.forEach((item, index) => validateNode(schema.items!, item, `${path}[${index}]`, errors));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function jsonType(value: unknown): string {
  if (Array.isArray(value)) return "array";
  if (value === null) return "null";
  return typeof value;
}

function deepEqual(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}
