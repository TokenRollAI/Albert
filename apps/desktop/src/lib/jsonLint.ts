/**
 * Lightweight JSON lint that returns a success marker, "empty", or an
 * error with a 1-based line/column derived from the raw input. Built on
 * top of the platform's native JSON.parse — we extract the position from
 * the error message (v8 / spidermonkey / jsc all report "at position N")
 * or fall back to a no-position generic error when the engine doesn't
 * surface one. No external dependencies.
 */
export type JsonLintResult =
  | { ok: true; empty: boolean }
  | { ok: false; message: string; line?: number; column?: number };

/**
 * Given a `position` byte offset into `source`, return the (line, column)
 * pair (1-based, column counts UTF-16 code units which matches what a
 * user sees in a textarea). Returns undefined if the offset is invalid.
 */
function offsetToLineColumn(
  source: string,
  position: number
): { line: number; column: number } | undefined {
  if (position < 0 || position > source.length) return undefined;
  let line = 1;
  let column = 1;
  for (let i = 0; i < position; i++) {
    if (source[i] === "\n") {
      line += 1;
      column = 1;
    } else {
      column += 1;
    }
  }
  return { line, column };
}

export function lintJson(source: string): JsonLintResult {
  if (source.trim().length === 0) {
    return { ok: true, empty: true };
  }
  try {
    JSON.parse(source);
    return { ok: true, empty: false };
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    // Node / V8 formats: "Unexpected token } in JSON at position 42"
    // Safari / WebKit: "JSON Parse error: ..."
    // Try to extract a numeric position; if found, convert to line/col.
    const match = message.match(/position (\d+)/i);
    if (match) {
      const offset = Number(match[1]);
      const loc = offsetToLineColumn(source, offset);
      if (loc) {
        return {
          ok: false,
          message,
          line: loc.line,
          column: loc.column
        };
      }
    }
    return { ok: false, message };
  }
}
