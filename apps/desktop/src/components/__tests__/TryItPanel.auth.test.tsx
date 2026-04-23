import { beforeEach, describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";
import {
  TryItPanel,
  placeholderForAuthHint
} from "../TryItPanel";
import type { CanonicalEndpoint, EndpointTab } from "../../types";

function makeTab(auth: CanonicalEndpoint["auth"]): EndpointTab {
  return {
    id: "tab-auth",
    collectionId: "col",
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

beforeEach(() => {
  window.localStorage.clear();
});

describe("placeholderForAuthHint", () => {
  test("bearer → 'Bearer '", () => {
    expect(
      placeholderForAuthHint({
        scheme: "http_bearer",
        header_name: "Authorization"
      })
    ).toBe("Bearer ");
  });
  test("basic → 'Basic '", () => {
    expect(
      placeholderForAuthHint({
        scheme: "http_basic",
        header_name: "Authorization"
      })
    ).toBe("Basic ");
  });
  test("oauth2 normalizes to Bearer", () => {
    expect(
      placeholderForAuthHint({
        scheme: "oauth2",
        header_name: "Authorization"
      })
    ).toBe("Bearer ");
  });
  test("api_key_header → empty string", () => {
    expect(
      placeholderForAuthHint({
        scheme: "api_key_header",
        header_name: "X-Api-Key"
      })
    ).toBe("");
  });
  test("other scheme → null (can't guess)", () => {
    expect(
      placeholderForAuthHint({
        scheme: "other",
        header_name: "Authorization"
      })
    ).toBe(null);
  });
});

describe("TryItPanel auth header seeding", () => {
  test("prefills an Authorization row when the endpoint declares bearer auth", () => {
    render(
      <TryItPanel
        tab={makeTab({
          scheme: "http_bearer",
          header_name: "Authorization",
          value_prefix: "Bearer ",
          description: null
        })}
        baseUrl={null}
      />
    );
    const keyInputs = screen.getAllByPlaceholderText(
      "Authorization"
    ) as HTMLInputElement[];
    const authRow = keyInputs.find(
      (input) => input.value === "Authorization"
    );
    expect(authRow).toBeDefined();
  });

  test("does not overwrite an existing Authorization header in the draft", () => {
    const routeKey = "GET /secret";
    window.localStorage.setItem(
      `albert.tryit.1:${routeKey}`,
      JSON.stringify({
        params: {},
        query: "",
        body: "",
        headers: [{ key: "Authorization", value: "Bearer existing-token" }]
      })
    );
    render(
      <TryItPanel
        tab={makeTab({
          scheme: "http_bearer",
          header_name: "Authorization",
          value_prefix: "Bearer ",
          description: null
        })}
        baseUrl={null}
      />
    );
    const valueInputs = screen.getAllByPlaceholderText(
      /Bearer/i
    ) as HTMLInputElement[];
    expect(valueInputs.some((i) => i.value === "Bearer existing-token")).toBe(
      true
    );
    // Only one Authorization row — no duplicate seeded.
    const keyInputs = screen.getAllByPlaceholderText(
      "Authorization"
    ) as HTMLInputElement[];
    const authRows = keyInputs.filter((i) => i.value === "Authorization");
    expect(authRows).toHaveLength(1);
  });

  test("does nothing for endpoints without an auth hint", () => {
    render(<TryItPanel tab={makeTab(null)} baseUrl={null} />);
    // Empty hint state shows the "No custom headers." copy.
    expect(screen.getByText(/No custom headers/i)).toBeTruthy();
  });
});
