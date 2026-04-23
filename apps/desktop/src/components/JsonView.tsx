import { useMemo } from "react";

interface JsonViewProps {
  value: unknown;
  className?: string;
}

type Token = { kind: TokenKind; text: string };
type TokenKind =
  | "brace"
  | "bracket"
  | "comma"
  | "colon"
  | "key"
  | "string"
  | "number"
  | "boolean"
  | "null"
  | "whitespace";

export function JsonView({ value, className }: JsonViewProps) {
  const source = useMemo(() => {
    try {
      return JSON.stringify(value, null, 2) ?? "null";
    } catch {
      return String(value ?? "");
    }
  }, [value]);

  const tokens = useMemo(() => tokenize(source), [source]);

  return (
    <pre className={className ? `code-block ${className}` : "code-block"}>
      {tokens.map((token, idx) => (
        <span key={idx} className={`jsont jsont--${token.kind}`}>
          {token.text}
        </span>
      ))}
    </pre>
  );
}

function tokenize(input: string): Token[] {
  const tokens: Token[] = [];
  let i = 0;
  while (i < input.length) {
    const ch = input[i];
    if (ch === "{" || ch === "}") {
      tokens.push({ kind: "brace", text: ch });
      i += 1;
      continue;
    }
    if (ch === "[" || ch === "]") {
      tokens.push({ kind: "bracket", text: ch });
      i += 1;
      continue;
    }
    if (ch === ",") {
      tokens.push({ kind: "comma", text: ch });
      i += 1;
      continue;
    }
    if (ch === ":") {
      tokens.push({ kind: "colon", text: ch });
      i += 1;
      continue;
    }
    if (ch === "\"") {
      const start = i;
      i += 1;
      while (i < input.length) {
        if (input[i] === "\\") {
          i += 2;
          continue;
        }
        if (input[i] === "\"") {
          i += 1;
          break;
        }
        i += 1;
      }
      const text = input.slice(start, i);
      // Look ahead to see whether this string is a key (followed by whitespace + colon).
      let j = i;
      while (j < input.length && /\s/.test(input[j])) j += 1;
      const kind: TokenKind = input[j] === ":" ? "key" : "string";
      tokens.push({ kind, text });
      continue;
    }
    if (/\s/.test(ch)) {
      const start = i;
      while (i < input.length && /\s/.test(input[i])) i += 1;
      tokens.push({ kind: "whitespace", text: input.slice(start, i) });
      continue;
    }
    if (ch === "-" || /[0-9]/.test(ch)) {
      const start = i;
      i += 1;
      while (i < input.length && /[0-9.eE+-]/.test(input[i])) i += 1;
      tokens.push({ kind: "number", text: input.slice(start, i) });
      continue;
    }
    if (input.startsWith("true", i)) {
      tokens.push({ kind: "boolean", text: "true" });
      i += 4;
      continue;
    }
    if (input.startsWith("false", i)) {
      tokens.push({ kind: "boolean", text: "false" });
      i += 5;
      continue;
    }
    if (input.startsWith("null", i)) {
      tokens.push({ kind: "null", text: "null" });
      i += 4;
      continue;
    }
    // Unknown — emit as whitespace-ish passthrough so we don't lose content.
    tokens.push({ kind: "whitespace", text: ch });
    i += 1;
  }
  return tokens;
}
