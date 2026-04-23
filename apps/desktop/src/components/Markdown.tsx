import type { ReactNode } from "react";

interface MarkdownProps {
  source: string;
}

/**
 * Intentionally tiny markdown renderer for endpoint descriptions.
 *
 * Supported inline spans: `code`, **bold**, *italic*, [link](url).
 * Block handling: paragraphs split on blank lines, line breaks preserved
 * within a paragraph via `\n`. No headings, no lists — endpoints rarely
 * need more than a short paragraph, and we want to keep this dependency-
 * free.
 */
export function Markdown({ source }: MarkdownProps) {
  if (!source) return null;
  const paragraphs = source.split(/\n\s*\n/);
  return (
    <>
      {paragraphs.map((paragraph, idx) => (
        <p key={idx} className="md-p">
          {renderInline(paragraph)}
        </p>
      ))}
    </>
  );
}

function renderInline(input: string): ReactNode[] {
  const nodes: ReactNode[] = [];
  let remaining = input;
  let key = 0;

  // Greedy left-to-right tokenizer. We look for whichever opening token
  // appears first; anything before it is pushed as plain text with
  // embedded `\n` replaced by <br/> so line breaks survive.
  while (remaining.length > 0) {
    const match = findNextToken(remaining);
    if (!match) {
      nodes.push(...splitLines(remaining, key));
      break;
    }
    if (match.index > 0) {
      nodes.push(...splitLines(remaining.slice(0, match.index), key));
      key = nodes.length;
    }
    const handler = MATCHERS[match.kind];
    const produced = handler.render(remaining.slice(match.index), key);
    if (!produced) {
      // Defensive: if the renderer can't parse what findNextToken claimed
      // to find, emit one char and continue so we don't loop forever.
      nodes.push(remaining.charAt(match.index));
      remaining = remaining.slice(match.index + 1);
      continue;
    }
    nodes.push(produced.node);
    remaining = remaining.slice(match.index + produced.consumed);
    key = nodes.length;
  }
  return nodes;
}

type TokenKind = "code" | "bold" | "italic" | "link";

interface Matcher {
  regex: RegExp;
  render: (
    slice: string,
    key: number
  ) => { node: ReactNode; consumed: number } | null;
}

const MATCHERS: Record<TokenKind, Matcher> = {
  code: {
    regex: /^`([^`]+)`/,
    render(slice, key) {
      const m = slice.match(this.regex);
      if (!m) return null;
      return {
        node: (
          <code key={key} className="md-code">
            {m[1]}
          </code>
        ),
        consumed: m[0].length
      };
    }
  },
  bold: {
    regex: /^\*\*([^*]+)\*\*/,
    render(slice, key) {
      const m = slice.match(this.regex);
      if (!m) return null;
      return {
        node: <strong key={key}>{m[1]}</strong>,
        consumed: m[0].length
      };
    }
  },
  italic: {
    regex: /^\*([^*]+)\*/,
    render(slice, key) {
      const m = slice.match(this.regex);
      if (!m) return null;
      return {
        node: <em key={key}>{m[1]}</em>,
        consumed: m[0].length
      };
    }
  },
  link: {
    regex: /^\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/,
    render(slice, key) {
      const m = slice.match(this.regex);
      if (!m) return null;
      return {
        node: (
          <a
            key={key}
            href={m[2]}
            target="_blank"
            rel="noreferrer noopener"
            className="md-link"
          >
            {m[1]}
          </a>
        ),
        consumed: m[0].length
      };
    }
  }
};

function findNextToken(
  input: string
): { index: number; kind: TokenKind } | null {
  let best: { index: number; kind: TokenKind } | null = null;
  for (const kind of Object.keys(MATCHERS) as TokenKind[]) {
    const regex = kind === "bold" ? /\*\*/ : kind === "italic" ? /\*/ : kind === "code" ? /`/ : /\[/;
    const idx = input.search(regex);
    if (idx < 0) continue;
    // Make sure the matcher actually succeeds starting from this index;
    // otherwise find a later occurrence by advancing past it.
    const slice = input.slice(idx);
    if (MATCHERS[kind].regex.test(slice)) {
      if (best == null || idx < best.index) {
        best = { index: idx, kind };
      }
    }
  }
  return best;
}

function splitLines(text: string, key: number): ReactNode[] {
  const parts = text.split("\n");
  const out: ReactNode[] = [];
  for (let i = 0; i < parts.length; i += 1) {
    if (parts[i]) out.push(parts[i]);
    if (i < parts.length - 1) out.push(<br key={`br-${key}-${i}`} />);
  }
  return out;
}
