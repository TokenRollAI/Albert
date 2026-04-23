import { useState } from "react";

export interface SchemaNodeShape {
  node_type?: string;
  description?: string | null;
  required?: boolean;
  nullable?: boolean;
  properties?: Record<string, SchemaNodeShape>;
  items?: SchemaNodeShape | null;
  enum_values?: unknown[];
  example?: unknown;
}

interface SchemaTreeProps {
  schema: SchemaNodeShape | null | undefined;
  depth?: number;
}

export function SchemaTree({ schema }: SchemaTreeProps) {
  if (!schema) {
    return <div className="schema-tree__empty">No schema.</div>;
  }
  return (
    <ul className="schema-tree">
      <SchemaRow name="root" node={schema} required />
    </ul>
  );
}

function SchemaRow({
  name,
  node,
  required
}: {
  name: string;
  node: SchemaNodeShape;
  required: boolean;
}) {
  const [open, setOpen] = useState<boolean>(true);
  const kind = node.node_type ?? "unknown";
  const hasChildren =
    (kind === "object" && node.properties && Object.keys(node.properties).length > 0) ||
    (kind === "array" && node.items);

  return (
    <li className={`schema-tree__row schema-tree__row--${kind}`}>
      <div className="schema-tree__head">
        {hasChildren ? (
          <button
            type="button"
            className="schema-tree__toggle"
            onClick={() => setOpen((prev) => !prev)}
            aria-label={open ? "Collapse" : "Expand"}
          >
            {open ? "▾" : "▸"}
          </button>
        ) : (
          <span className="schema-tree__toggle schema-tree__toggle--leaf">•</span>
        )}
        <span className="schema-tree__name">{name}</span>
        <span className={`schema-tree__kind schema-tree__kind--${kind}`}>
          {kind}
          {node.nullable ? "?" : ""}
        </span>
        {required ? (
          <span className="schema-tree__required">required</span>
        ) : null}
        {node.enum_values && node.enum_values.length > 0 ? (
          <span className="schema-tree__enum" title={JSON.stringify(node.enum_values)}>
            enum {node.enum_values.length}
          </span>
        ) : null}
      </div>
      {node.description ? (
        <div className="schema-tree__desc">{node.description}</div>
      ) : null}
      {hasChildren && open ? (
        <ul className="schema-tree__children">
          {kind === "object" && node.properties
            ? Object.entries(node.properties).map(([childName, childNode]) => (
                <SchemaRow
                  key={childName}
                  name={childName}
                  node={childNode}
                  required={Boolean(childNode.required)}
                />
              ))
            : null}
          {kind === "array" && node.items ? (
            <SchemaRow name="[item]" node={node.items} required={false} />
          ) : null}
        </ul>
      ) : null}
    </li>
  );
}
