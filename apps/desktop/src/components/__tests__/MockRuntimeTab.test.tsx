import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { MockRuntimeTab } from "../MockRuntimeTab";
import type { GatewayStatus } from "../../types";

type StatusOverrides = Partial<Omit<GatewayStatus, "config">> & {
  config?: Partial<GatewayStatus["config"]>;
};

function status(overrides: StatusOverrides = {}): GatewayStatus {
  const base: GatewayStatus = {
    running: true,
    bind_address: "127.0.0.1:4317",
    route_count: 0,
    started_at_epoch_ms: 1700000000000,
    routes: [],
    config: {
      host: "127.0.0.1",
      port: 4317,
      cors_enabled: true,
      example_overrides: {},
      use_request_cache: false,
      request_cache_entries: {},
      default_latency_ms: null,
      latency_overrides: {},
      error_rate: 0
    }
  };
  return {
    ...base,
    ...overrides,
    config: {
      ...base.config,
      ...(overrides.config ?? {})
    }
  };
}

function renderRuntime(
  gatewayStatus: GatewayStatus,
  onToggleRequestCache = vi.fn().mockResolvedValue(undefined),
  onReloadRequestCache = vi.fn().mockResolvedValue(undefined)
) {
  return render(
    <MockRuntimeTab
      status={gatewayStatus}
      connected={true}
      busy={false}
      error={null}
      savedPreferences={null}
      onStart={vi.fn()}
      onStop={vi.fn()}
      onApplyChaos={vi.fn()}
      onToggleEnforceRequestBodies={vi.fn()}
      onApplyRateLimits={vi.fn()}
      onApplyStatusOverrides={vi.fn()}
      onApplyResponseHeaders={vi.fn()}
      onSeedRequiredHeadersFromHints={vi.fn()}
      onToggleRequestCache={onToggleRequestCache}
      onReloadRequestCache={onReloadRequestCache}
    />
  );
}

describe("MockRuntimeTab", () => {
  test("toggles request cache routing while the gateway is running", async () => {
    const onToggleRequestCache = vi.fn().mockResolvedValue(undefined);
    renderRuntime(status(), onToggleRequestCache);

    const checkbox = screen.getByRole("checkbox", {
      name: /Serve matching cached Try-it responses/i
    });
    expect((checkbox as HTMLInputElement).checked).toBe(false);

    fireEvent.click(checkbox);

    await waitFor(() =>
      expect(onToggleRequestCache).toHaveBeenCalledWith(true)
    );
  });

  test("disables request cache routing toggle while the gateway is idle", () => {
    renderRuntime(
      status({
        running: false,
        bind_address: null,
        started_at_epoch_ms: null
      })
    );

    const checkbox = screen.getByRole("checkbox", {
      name: /Serve matching cached Try-it responses/i
    }) as HTMLInputElement;
    expect(checkbox.disabled).toBe(true);
  });

  test("shows injected request cache entry count", () => {
    renderRuntime(
      status({
        config: {
          use_request_cache: true,
          request_cache_entries: {
            fp_a: { status: 200 },
            fp_b: { status: 202 }
          }
        }
      })
    );

    expect(screen.getByText("2 cached")).toBeTruthy();
  });

  test("reloads request cache routing while enabled", async () => {
    const onReloadRequestCache = vi.fn().mockResolvedValue(undefined);
    renderRuntime(
      status({
        config: {
          use_request_cache: true,
          request_cache_entries: {
            fp_a: { status: 200 }
          }
        }
      }),
      vi.fn().mockResolvedValue(undefined),
      onReloadRequestCache
    );

    fireEvent.click(
      screen.getByRole("button", { name: /Reload request cache/i })
    );

    await waitFor(() => expect(onReloadRequestCache).toHaveBeenCalledTimes(1));
  });

  test("disables request cache reload while routing is off", () => {
    renderRuntime(status());

    const button = screen.getByRole("button", {
      name: /Reload request cache/i
    }) as HTMLButtonElement;
    expect(button.disabled).toBe(true);
  });
});
