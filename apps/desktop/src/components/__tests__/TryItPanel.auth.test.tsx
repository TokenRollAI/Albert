import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import {
  TryItPanel,
  formatBytes,
  placeholderForAuthHint
} from "../TryItPanel";
import type { CanonicalEndpoint, EndpointTab, RequestCacheEntry } from "../../types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

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

function makeSchemaTab(): EndpointTab {
  const tab = makeTab(null);
  return {
    ...tab,
    endpoint: {
      ...tab.endpoint,
      responses: [
        {
          status_code: "200",
          content_type: "application/json",
          schema: {
            node_type: "object",
            properties: {
              id: { node_type: "string", required: true }
            }
          }
        }
      ]
    }
  };
}

function makePostTab(): EndpointTab {
  const tab = makeSchemaTab();
  return {
    ...tab,
    id: "tab-post",
    method: "POST",
    path: "/secret",
    endpoint: {
      ...tab.endpoint,
      method: "POST",
      path: "/secret",
      request_body: {
        content_type: "application/json",
        required: true,
        schema: {
          node_type: "object",
          properties: {
            sku: { node_type: "string", required: true }
          }
        }
      }
    }
  };
}

function makeCacheEntry(
  overrides: Partial<RequestCacheEntry> = {}
): RequestCacheEntry {
  return {
    id: "cache-1",
    collection_id: "col",
    method: "GET",
    path: "/secret",
    fingerprint: "abc123",
    request_snapshot: {
      query: "",
      headers: {},
      body: null
    },
    response_snapshot: {
      status: 200,
      headers: { "content-type": "application/json" },
      body: { cached: true },
      elapsed_ms: 7,
      size_bytes: 15
    },
    hit_count: 2,
    first_seen_at: "1800000000",
    last_seen_at: "1800000123",
    ...overrides
  };
}

beforeEach(() => {
  window.localStorage.clear();
});

afterEach(() => {
  vi.useRealTimers();
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

describe("TryItPanel captured responses", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [];
      }
      if (command === "save_request_cache") {
        return makeCacheEntry({ hit_count: 1, response_snapshot: {} });
      }
      if (command === "validate_mock_payload") {
        throw new Error("validator unavailable in this test");
      }
      return null;
    });
  });

  test("saves the latest JSON response as a mock example", async () => {
    const onSaveResponseAsExample = vi.fn().mockResolvedValue(null);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 201,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onSaveResponseAsExample={onSaveResponseAsExample}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));
    expect(await screen.findByText(/201 ·/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Save as mock/i }));

    await waitFor(() =>
      expect(onSaveResponseAsExample).toHaveBeenCalledWith(
        expect.objectContaining({ id: "tab-auth" }),
        {
          kind: "success",
          title: "Success from Try-it",
          payload: { ok: true },
          note: expect.stringContaining("Captured from Try-it response (201")
        }
      )
    );
    expect(await screen.findByRole("button", { name: /Saved/i })).toBeDefined();
  });

  test("stores a request fingerprint cache entry after a successful send", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));
    expect(await screen.findByText(/^cached$/i)).toBeDefined();

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("save_request_cache", {
        args: {
          collection_id: "col",
          method: "GET",
          path: "/secret",
          request_snapshot: {
            query: "",
            headers: {},
            body: null
          },
          response_snapshot: {
            status: 200,
            headers: expect.objectContaining({
              "content-type": expect.stringContaining("application/json")
            }),
            body: { ok: true },
            elapsed_ms: expect.any(Number),
            size_bytes: expect.any(Number)
          },
          database_url: null
        }
      })
    );
  });

  test("loads cached fingerprints for the active endpoint", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [makeCacheEntry()];
      }
      if (command === "validate_mock_payload") {
        throw new Error("validator unavailable in this test");
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
      />
    );

    expect(await screen.findByText(/Cached fingerprints \(1\)/i)).toBeDefined();
    expect(await screen.findByText(/hit ×2/i)).toBeDefined();
    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("list_request_cache", {
        args: {
          collection_id: "col",
          method: "GET",
          path: "/secret",
          limit: 5,
          database_url: null
        }
      })
    );
  });

  test("marks old cached fingerprints as stale", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1800090000 * 1000);
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [makeCacheEntry({ last_seen_at: "1800000000" })];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
      />
    );

    expect(await screen.findByText(/stale · 25h old/i)).toBeDefined();
  });

  test("replays cached request snapshots into the Try-it draft", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [
          makeCacheEntry({
            method: "POST",
            request_snapshot: {
              query: "status=paid",
              headers: { "x-trace": "abc" },
              body: { sku: "cached-sku" }
            }
          })
        ];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makePostTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
      />
    );

    expect(await screen.findByText(/Cached fingerprints \(1\)/i)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Replay/i }));

    const rawQuery = screen.getByLabelText("Raw query string") as HTMLInputElement;
    expect(rawQuery.value).toBe("status=paid");
    expect(
      (screen.getByPlaceholderText("Authorization") as HTMLInputElement).value
    ).toBe("x-trace");
    expect(
      (screen.getByPlaceholderText("Bearer …") as HTMLInputElement).value
    ).toBe("abc");
    expect(screen.getByDisplayValue(/cached-sku/i)).toBeDefined();
  });

  test("saves a cached response as a mock example", async () => {
    const onSaveResponseAsExample = vi.fn().mockResolvedValue(null);
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [makeCacheEntry()];
      }
      if (command === "validate_mock_payload") {
        throw new Error("validator unavailable in this test");
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onSaveResponseAsExample={onSaveResponseAsExample}
      />
    );

    expect(await screen.findByText(/Cached fingerprints \(1\)/i)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /^Save$/i }));

    await waitFor(() =>
      expect(onSaveResponseAsExample).toHaveBeenCalledWith(
        expect.anything(),
        expect.objectContaining({
          kind: "success",
          title: "Success from cache",
          payload: { cached: true },
          note: expect.stringContaining("Captured from request cache (200")
        })
      )
    );
    expect(await screen.findByRole("button", { name: /Saved/i })).toBeDefined();
  });

  test("refreshes a mock with AI using cached request context", async () => {
    const onGenerateFromCache = vi.fn().mockResolvedValue(null);
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [
          makeCacheEntry({
            request_snapshot: {
              query: "status=paid",
              headers: { "x-trace": "abc" },
              body: null
            }
          })
        ];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onGenerateFromCache={onGenerateFromCache}
      />
    );

    expect(await screen.findByText(/Cached fingerprints \(1\)/i)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /AI refresh/i }));

    await waitFor(() =>
      expect(onGenerateFromCache).toHaveBeenCalledWith(
        expect.objectContaining({ id: "tab-auth" }),
        "success",
        {
          request_snapshot: {
            query: "status=paid",
            headers: { "x-trace": "abc" },
            body: null
          },
          response_snapshot: {
            status: 200,
            headers: { "content-type": "application/json" },
            body: { cached: true },
            elapsed_ms: 7,
            size_bytes: 15
          },
          note: expect.stringContaining("request cache abc123")
        }
      )
    );
    expect(await screen.findByRole("button", { name: /Refreshed/i })).toBeDefined();
  });

  test("previews the AI prompt using cached request context", async () => {
    const onPreviewPromptFromCache = vi.fn().mockResolvedValue(null);
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [
          makeCacheEntry({
            request_snapshot: {
              query: "status=failed",
              headers: { "x-trace": "xyz" },
              body: null
            },
            response_snapshot: {
              status: 500,
              headers: { "content-type": "application/json" },
              body: { error: "upstream" },
              elapsed_ms: 12,
              size_bytes: 21
            }
          })
        ];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onPreviewPromptFromCache={onPreviewPromptFromCache}
      />
    );

    expect(await screen.findByText(/Cached fingerprints \(1\)/i)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /^Prompt$/i }));

    await waitFor(() =>
      expect(onPreviewPromptFromCache).toHaveBeenCalledWith(
        expect.objectContaining({ id: "tab-auth" }),
        "error",
        {
          request_snapshot: {
            query: "status=failed",
            headers: { "x-trace": "xyz" },
            body: null
          },
          response_snapshot: {
            status: 500,
            headers: { "content-type": "application/json" },
            body: { error: "upstream" },
            elapsed_ms: 12,
            size_bytes: 21
          },
          note: expect.stringContaining("request cache abc123")
        }
      )
    );
  });

  test("refreshes a mock with AI using the latest Try-it response context", async () => {
    const onGenerateFromCache = vi.fn().mockResolvedValue(null);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: true, id: "latest" }), {
        status: 201,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onSaveResponseAsExample={vi.fn()}
        onGenerateFromCache={onGenerateFromCache}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));
    expect(await screen.findByText(/201 ·/)).toBeDefined();
    fireEvent.click(
      screen.getByRole("button", { name: /AI refresh latest/i })
    );

    await waitFor(() =>
      expect(onGenerateFromCache).toHaveBeenCalledWith(
        expect.objectContaining({ id: "tab-auth" }),
        "success",
        expect.objectContaining({
          request_snapshot: {
            query: "",
            headers: {},
            body: null
          },
          response_snapshot: expect.objectContaining({
            status: 201,
            body: { ok: true, id: "latest" },
            elapsed_ms: expect.any(Number),
            size_bytes: expect.any(Number)
          }),
          note: expect.stringContaining("latest Try-it response 201")
        })
      )
    );
    expect(await screen.findByRole("button", { name: /Refreshed/i })).toBeDefined();
  });

  test("previews the AI prompt using the latest Try-it response context", async () => {
    const onPreviewPromptFromCache = vi.fn().mockResolvedValue(null);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ error: "denied" }), {
        status: 403,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onSaveResponseAsExample={vi.fn()}
        onPreviewPromptFromCache={onPreviewPromptFromCache}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));
    expect(await screen.findByText(/403 ·/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /^Prompt latest$/i }));

    await waitFor(() =>
      expect(onPreviewPromptFromCache).toHaveBeenCalledWith(
        expect.objectContaining({ id: "tab-auth" }),
        "error",
        expect.objectContaining({
          request_snapshot: {
            query: "",
            headers: {},
            body: null
          },
          response_snapshot: expect.objectContaining({
            status: 403,
            body: { error: "denied" }
          }),
          note: expect.stringContaining("latest Try-it response 403")
        })
      )
    );
  });

  test("disables AI refresh when provider settings are incomplete", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [makeCacheEntry()];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onGenerateFromCache={vi.fn()}
        canGenerateFromCache={false}
      />
    );

    const refresh = await screen.findByRole("button", { name: /AI refresh/i });
    expect((refresh as HTMLButtonElement).disabled).toBe(true);
  });

  test("batch refreshes stale cached fingerprints with AI context", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1800090000 * 1000);
    const onGenerateFromCache = vi.fn().mockResolvedValue(null);
    const oldSuccess = makeCacheEntry({
      id: "old-success",
      fingerprint: "old-success-fp",
      last_seen_at: "1800000000",
      request_snapshot: {
        query: "status=paid",
        headers: { "x-trace": "old-success" },
        body: null
      }
    });
    const oldError = makeCacheEntry({
      id: "old-error",
      fingerprint: "old-error-fp",
      last_seen_at: "1800000100",
      request_snapshot: {
        query: "status=failed",
        headers: { "x-trace": "old-error" },
        body: null
      },
      response_snapshot: {
        status: 500,
        headers: { "content-type": "application/json" },
        body: { error: "upstream" },
        elapsed_ms: 12,
        size_bytes: 21
      }
    });
    const fresh = makeCacheEntry({
      id: "fresh-cache",
      fingerprint: "fresh-fp",
      last_seen_at: "1800089900"
    });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [oldSuccess, oldError, fresh];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onGenerateFromCache={onGenerateFromCache}
      />
    );

    expect(await screen.findByText(/Cached fingerprints \(3\)/i)).toBeDefined();
    fireEvent.click(
      screen.getByRole("button", { name: /AI refresh stale \(2\)/i })
    );

    await waitFor(() => expect(onGenerateFromCache).toHaveBeenCalledTimes(2));
    expect(onGenerateFromCache).toHaveBeenNthCalledWith(
      1,
      expect.objectContaining({ id: "tab-auth" }),
      "success",
      expect.objectContaining({
        request_snapshot: {
          query: "status=paid",
          headers: { "x-trace": "old-success" },
          body: null
        },
        response_snapshot: expect.objectContaining({
          status: 200,
          body: { cached: true }
        }),
        note: expect.stringContaining("request cache old-success-fp")
      })
    );
    expect(onGenerateFromCache).toHaveBeenNthCalledWith(
      2,
      expect.objectContaining({ id: "tab-auth" }),
      "error",
      expect.objectContaining({
        request_snapshot: {
          query: "status=failed",
          headers: { "x-trace": "old-error" },
          body: null
        },
        response_snapshot: expect.objectContaining({
          status: 500,
          body: { error: "upstream" }
        }),
        note: expect.stringContaining("request cache old-error-fp")
      })
    );
    expect(onGenerateFromCache).not.toHaveBeenCalledWith(
      expect.anything(),
      expect.anything(),
      expect.objectContaining({
        note: expect.stringContaining("fresh-fp")
      })
    );
    expect(
      await screen.findByText(/Refreshed 2\/2 stale cached fingerprints/i)
    ).toBeDefined();
  });

  test("batch refresh counts only refreshable stale cached fingerprints", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1800090000 * 1000);
    const onGenerateFromCache = vi.fn().mockResolvedValue(null);
    const refreshable = makeCacheEntry({
      id: "refreshable-cache",
      fingerprint: "refreshable-fp",
      last_seen_at: "1800000000"
    });
    const invalid = makeCacheEntry({
      id: "invalid-cache",
      fingerprint: "invalid-fp",
      last_seen_at: "1800000100",
      response_snapshot: {
        status: 200,
        headers: {},
        body: "<invalid JSON>",
        elapsed_ms: 4,
        size_bytes: 1
      }
    });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [refreshable, invalid];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onGenerateFromCache={onGenerateFromCache}
      />
    );

    const refresh = await screen.findByRole("button", {
      name: /AI refresh stale \(1\)/i
    });
    fireEvent.click(refresh);

    await waitFor(() => expect(onGenerateFromCache).toHaveBeenCalledTimes(1));
    expect(onGenerateFromCache).toHaveBeenCalledWith(
      expect.anything(),
      "success",
      expect.objectContaining({
        note: expect.stringContaining("request cache refreshable-fp")
      })
    );
    expect(screen.getByText(/Refresh queue/i)).toBeDefined();
    expect(screen.getByText(/2 stale · 1 refreshable/i)).toBeDefined();
  });

  test("previews the first refreshable stale cached prompt from the queue", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1800090000 * 1000);
    const onPreviewPromptFromCache = vi.fn().mockResolvedValue(null);
    const first = makeCacheEntry({
      id: "first-stale",
      fingerprint: "first-stale-fp",
      last_seen_at: "1800000000",
      request_snapshot: {
        query: "tier=gold",
        headers: { "x-trace": "first" },
        body: null
      }
    });
    const second = makeCacheEntry({
      id: "second-stale",
      fingerprint: "second-stale-fp",
      last_seen_at: "1800000100"
    });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [first, second];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onPreviewPromptFromCache={onPreviewPromptFromCache}
      />
    );

    expect(await screen.findByText(/Refresh queue/i)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Preview first/i }));

    await waitFor(() =>
      expect(onPreviewPromptFromCache).toHaveBeenCalledWith(
        expect.anything(),
        "success",
        expect.objectContaining({
          request_snapshot: {
            query: "tier=gold",
            headers: { "x-trace": "first" },
            body: null
          },
          note: expect.stringContaining("request cache first-stale-fp")
        })
      )
    );
  });

  test("disables batch stale AI refresh when provider settings are incomplete", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1800090000 * 1000);
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [makeCacheEntry({ last_seen_at: "1800000000" })];
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onGenerateFromCache={vi.fn()}
        canGenerateFromCache={false}
      />
    );

    const refresh = await screen.findByRole("button", {
      name: /AI refresh stale \(1\)/i
    });
    expect((refresh as HTMLButtonElement).disabled).toBe(true);
  });

  test("removes a single cached fingerprint and refreshes the list", async () => {
    const entry = makeCacheEntry();
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [entry];
      }
      if (command === "delete_request_cache") {
        return true;
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
      />
    );

    expect(await screen.findByText(/Cached fingerprints \(1\)/i)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /^Remove$/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("delete_request_cache", {
        args: {
          collection_id: "col",
          method: "GET",
          path: "/secret",
          cache_id: "cache-1",
          database_url: null
        }
      })
    );
    await waitFor(() =>
      expect(
        vi
          .mocked(invoke)
          .mock.calls.filter(([command]) => command === "list_request_cache")
      ).toHaveLength(2)
    );
  });

  test("clears stale cached fingerprints for the active endpoint", async () => {
    vi.spyOn(Date, "now").mockReturnValue(1800090000 * 1000);
    const oldEntry = makeCacheEntry({
      id: "old-cache",
      fingerprint: "old123",
      last_seen_at: "1800000000"
    });
    const freshEntry = makeCacheEntry({
      id: "fresh-cache",
      fingerprint: "fresh123",
      last_seen_at: "1800089900"
    });
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return [oldEntry, freshEntry];
      }
      if (command === "delete_stale_request_cache") {
        return 1;
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
      />
    );

    expect(await screen.findByText(/Cached fingerprints \(2\)/i)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Clear stale \(1\)/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("delete_stale_request_cache", {
        args: {
          collection_id: "col",
          method: "GET",
          path: "/secret",
          stale_before_epoch_seconds: 1800003600,
          database_url: null
        }
      })
    );
    await waitFor(() =>
      expect(
        vi
          .mocked(invoke)
          .mock.calls.filter(([command]) => command === "list_request_cache")
      ).toHaveLength(2)
    );
  });

  test("ignores non-array cache-list responses from older backends", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "list_request_cache") {
        return { unexpected: true };
      }
      return null;
    });
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
      />
    );

    await waitFor(() =>
      expect(screen.queryByText(/Cached fingerprints/i)).toBeNull()
    );
    expect(screen.getByRole("button", { name: /Send GET/i })).toBeDefined();
  });

  test("surfaces repeated request cache hits", async () => {
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "save_request_cache") {
        return {
          id: "cache-1",
          collection_id: "col",
          method: "GET",
          path: "/secret",
          fingerprint: "abc123",
          request_snapshot: {},
          response_snapshot: {},
          hit_count: 3,
          first_seen_at: "1800000000",
          last_seen_at: "1800000123"
        };
      }
      return { valid: true, errors: [] };
    });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));

    expect(await screen.findByText(/cache hit ×3/i)).toBeDefined();
    expect(await screen.findByText(/Request fingerprint cached/i)).toBeDefined();
  });

  test("offers to reload cache routing after saving a new fingerprint", async () => {
    const onReloadRequestCache = vi.fn().mockResolvedValue(undefined);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        requestCacheRoutingEnabled={true}
        onReloadRequestCache={onReloadRequestCache}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));

    expect(
      await screen.findByText(/Reload routing to serve this cached response immediately/i)
    ).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Reload routing/i }));

    await waitFor(() => expect(onReloadRequestCache).toHaveBeenCalledTimes(1));
    expect(
      await screen.findByText(/Request cache routing reloaded/i)
    ).toBeDefined();
  });

  test("does not show cache routing reload when routing is disabled", async () => {
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ ok: true }), {
        status: 200,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        requestCacheRoutingEnabled={false}
        onReloadRequestCache={vi.fn().mockResolvedValue(undefined)}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));

    expect(await screen.findByText(/^cached$/i)).toBeDefined();
    expect(screen.queryByText(/Reload routing to serve/i)).toBeNull();
  });

  test("maps captured 4xx responses to the error mock kind", async () => {
    const onSaveResponseAsExample = vi.fn().mockResolvedValue(null);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ error: "nope" }), {
        status: 404,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeTab(null)}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onSaveResponseAsExample={onSaveResponseAsExample}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));
    expect(await screen.findByText(/404 ·/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Save as mock/i }));

    await waitFor(() =>
      expect(onSaveResponseAsExample).toHaveBeenCalledWith(
        expect.anything(),
        expect.objectContaining({
          kind: "error",
          title: "Error from Try-it",
          payload: { error: "nope" }
        })
      )
    );
  });

  test("lets users override the captured mock kind before saving", async () => {
    const onSaveResponseAsExample = vi.fn().mockResolvedValue(null);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ error: "nope" }), {
        status: 404,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onSaveResponseAsExample={onSaveResponseAsExample}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));
    expect(await screen.findByText(/404 ·/)).toBeDefined();
    const select = screen.getByLabelText(
      /Mock kind for captured response/i
    ) as HTMLSelectElement;
    expect(select.value).toBe("error");

    fireEvent.change(select, { target: { value: "success" } });
    fireEvent.click(screen.getByRole("button", { name: /Save as mock/i }));

    expect(await screen.findByText(/does not match the success schema/i)).toBeDefined();
    await waitFor(() =>
      expect(onSaveResponseAsExample).toHaveBeenCalledWith(
        expect.anything(),
        expect.objectContaining({
          kind: "success",
          title: "Success from Try-it",
          payload: { error: "nope" }
        })
      )
    );
  });

  test("warns when captured response does not match the selected response schema", async () => {
    const onSaveResponseAsExample = vi.fn().mockResolvedValue(null);
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ active: true }), {
        status: 200,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onSaveResponseAsExample={onSaveResponseAsExample}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));
    expect(await screen.findByText(/200 ·/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Save as mock/i }));

    expect(await screen.findByText(/does not match the success schema/i)).toBeDefined();
    await waitFor(() =>
      expect(onSaveResponseAsExample).toHaveBeenCalledWith(
        expect.anything(),
        expect.objectContaining({
          kind: "success",
          payload: { active: true },
          note: expect.stringContaining("schema mismatch")
        })
      )
    );
  });

  test("uses the Tauri canonical validator before falling back to frontend checks", async () => {
    const onSaveResponseAsExample = vi.fn().mockResolvedValue(null);
    vi.mocked(invoke).mockImplementation(async (command: string) => {
      if (command === "save_request_cache") {
        return {
          id: "cache-1",
          collection_id: "col",
          method: "GET",
          path: "/secret",
          fingerprint: "abc123",
          request_snapshot: {},
          response_snapshot: {},
          hit_count: 1,
          first_seen_at: "1800000000",
          last_seen_at: "1800000000"
        };
      }
      if (command === "validate_mock_payload") {
        return {
          valid: false,
          errors: ["$.id: required property missing from Rust"]
        };
      }
      return null;
    });
    vi.spyOn(globalThis, "fetch").mockResolvedValue(
      new Response(JSON.stringify({ active: true }), {
        status: 200,
        headers: { "content-type": "application/json" }
      })
    );
    render(
      <TryItPanel
        tab={makeSchemaTab()}
        baseUrl="http://127.0.0.1:4317"
        connected={true}
        onSaveResponseAsExample={onSaveResponseAsExample}
      />
    );

    fireEvent.click(screen.getByRole("button", { name: /Send GET/i }));
    expect(await screen.findByText(/200 ·/)).toBeDefined();
    fireEvent.click(screen.getByRole("button", { name: /Save as mock/i }));

    await waitFor(() =>
      expect(invoke).toHaveBeenCalledWith("validate_mock_payload", {
        args: {
          schema: expect.objectContaining({ node_type: "object" }),
          payload: { active: true }
        }
      })
    );
    expect(
      await screen.findByText(/required property missing from Rust/i)
    ).toBeDefined();
  });
});
