import { beforeEach, describe, expect, test } from "vitest";
import { render, screen } from "@testing-library/react";
import {
  TryItPanel,
  formatBytes,
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

describe("formatBytes", () => {
  test("under 1 kB stays in bytes", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(42)).toBe("42 B");
    expect(formatBytes(1023)).toBe("1023 B");
  });
  test("under 1 MB uses kB with one decimal when < 10", () => {
    expect(formatBytes(1024)).toBe("1.0 kB");
    expect(formatBytes(2048)).toBe("2.0 kB");
    expect(formatBytes(1024 * 15)).toBe("15 kB");
  });
  test(">= 1 MB uses MB", () => {
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(1024 * 1024 * 8)).toBe("8.0 MB");
  });
  test("negative or non-finite input is safe", () => {
    expect(formatBytes(-1)).toBe("0 B");
    expect(formatBytes(Number.NaN)).toBe("0 B");
  });
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
