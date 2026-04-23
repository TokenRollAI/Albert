import { describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";
import { RequestPanel } from "../RequestPanel";
import type { CanonicalEndpoint, EndpointTab } from "../../types";

function makeTab(auth: CanonicalEndpoint["auth"]): EndpointTab {
  return {
    id: "tab-1",
    collectionId: "col-1",
    collectionName: "secure",
    method: "GET",
    path: "/secret",
    endpoint: {
      method: "GET",
      path: "/secret",
      tags: [],
      parameters: [],
      responses: [],
      examples: [],
      auth
    },
    inspector: "params",
    example: "success"
  };
}

function authText(): string {
  return (
    screen.getByRole("note", { name: /endpoint auth requirement/i })
      .textContent ?? ""
  );
}

describe("RequestPanel auth chip", () => {
  test("shows Bearer hint for http_bearer scheme", () => {
    render(
      <RequestPanel
        tab={makeTab({
          scheme: "http_bearer",
          header_name: "Authorization",
          value_prefix: "Bearer ",
          description: "Signed JWT"
        })}
        onSelectInspector={() => {}}
      />
    );
    expect(authText()).toMatch(/Authorization: Bearer/);
    expect(authText()).toContain("Signed JWT");
  });

  test("shows api-key hint for api_key_header scheme", () => {
    render(
      <RequestPanel
        tab={makeTab({
          scheme: "api_key_header",
          header_name: "X-Api-Key",
          value_prefix: null,
          description: null
        })}
        onSelectInspector={() => {}}
      />
    );
    expect(authText()).toMatch(/X-Api-Key: <api key>/);
  });

  test("normalizes OAuth2 schemes to a Bearer hint labeled OAuth2", () => {
    render(
      <RequestPanel
        tab={makeTab({
          scheme: "oauth2",
          header_name: "Authorization",
          value_prefix: "Bearer ",
          description: null
        })}
        onSelectInspector={() => {}}
      />
    );
    expect(authText()).toMatch(/Authorization: Bearer/);
    expect(authText()).toContain("OAuth2");
  });

  test("renders no chip when the endpoint has no auth hint", () => {
    render(
      <RequestPanel tab={makeTab(null)} onSelectInspector={() => {}} />
    );
    expect(
      screen.queryByRole("note", { name: /endpoint auth requirement/i })
    ).toBeNull();
  });
});
