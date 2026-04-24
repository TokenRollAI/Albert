import { useState } from "react";
import { Icon } from "./Icon";
import type { EndpointTab } from "../types";

interface UrlBarProps {
  tab: EndpointTab;
  disabled: boolean;
  baseUrl?: string | null;
}

/**
 * Build a reasonably-clean curl one-liner for the active endpoint.
 *
 * Uses the running mock base URL when one is available so the command
 * actually works against the local gateway. Otherwise it falls back to
 * a `https://api.example.com` placeholder so users can see the shape.
 *
 * Exported so the command palette can reuse the same formatter for its
 * "Copy cURL for active endpoint" action.
 */
export function buildCurlCommand(tab: EndpointTab, baseUrl: string | null): string {
  const method = tab.method.toUpperCase();
  const resolvedBase = baseUrl ?? "https://api.example.com";
  const parts: string[] = [`curl -X ${method}`];
  const url = `${resolvedBase.replace(/\/$/, "")}${tab.endpoint.path}`;
  parts.push(`"${url}"`);

  const headerParams = tab.endpoint.parameters.filter(
    (p) => p.location === "header"
  );
  for (const header of headerParams) {
    const exampleValue =
      typeof header.schema.properties === "object" &&
      header.schema.properties !== null
        ? "<value>"
        : "<value>";
    parts.push(`-H "${header.name}: ${exampleValue}"`);
  }

  if (tab.endpoint.request_body) {
    parts.push(`-H "Content-Type: ${tab.endpoint.request_body.content_type}"`);
    parts.push(`-d '{ "example": true }'`);
  }

  return parts.join(" \\\n  ");
}

export function UrlBar({ tab, disabled, baseUrl = null }: UrlBarProps) {
  const [copied, setCopied] = useState(false);

  async function copyCurl() {
    try {
      await navigator.clipboard.writeText(buildCurlCommand(tab, baseUrl));
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1400);
    } catch {
      /* ignore */
    }
  }

  return (
    <div className="urlbar">
      <div className="urlbar__input">
        <span
          className={`method method--${tab.method.toLowerCase()} method--chip`}
        >
          {tab.method}
        </span>
        <span className="urlbar__path" title={tab.path}>
          {tab.path}
        </span>
        <span className="urlbar__summary">
          {tab.endpoint.summary ?? tab.endpoint.operation_id ?? "endpoint"}
        </span>
      </div>

      <button
        type="button"
        className="btn btn--secondary btn--sm"
        onClick={copyCurl}
        disabled={disabled}
        title={
          baseUrl
            ? `Copy a curl command targeting ${baseUrl}`
            : "Copy a curl command template (start the mock server to target it)"
        }
      >
        <Icon name="copy" size={12} />
        <span>{copied ? "Copied!" : "Copy as cURL"}</span>
      </button>
    </div>
  );
}
