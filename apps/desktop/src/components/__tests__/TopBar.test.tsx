import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, test, vi } from "vitest";
import { TopBar } from "../TopBar";

function renderTopBar(overrides = {}) {
  const props = {
    workspace: "Orders",
    theme: "dark" as const,
    onToggleTheme: vi.fn(),
    onWorkspaceClick: vi.fn(),
    onImportClick: vi.fn(),
    onMockServerClick: vi.fn(),
    onProvidersClick: vi.fn(),
    onExportAll: vi.fn(),
    canExportAll: true,
    gatewayRunning: false,
    gatewayBind: null,
    ...overrides
  };
  render(<TopBar {...props} />);
  return props;
}

describe("TopBar", () => {
  test("opens workspace collections from the brand area", () => {
    const props = renderTopBar();
    fireEvent.click(
      screen.getByRole("button", { name: "Open workspace collections" })
    );
    expect(props.onWorkspaceClick).toHaveBeenCalledTimes(1);
  });

  test("wires primary action buttons", () => {
    const props = renderTopBar();
    fireEvent.click(screen.getByRole("button", { name: "Import" }));
    fireEvent.click(screen.getByRole("button", { name: "Mock Server" }));
    fireEvent.click(screen.getByRole("button", { name: "Providers" }));
    fireEvent.click(screen.getByRole("button", { name: "Export all collections" }));

    expect(props.onImportClick).toHaveBeenCalledTimes(1);
    expect(props.onMockServerClick).toHaveBeenCalledTimes(1);
    expect(props.onProvidersClick).toHaveBeenCalledTimes(1);
    expect(props.onExportAll).toHaveBeenCalledTimes(1);
  });
});
