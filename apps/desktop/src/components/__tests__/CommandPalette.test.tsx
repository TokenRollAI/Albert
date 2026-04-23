import { describe, expect, test, vi } from "vitest";
import { act, fireEvent, render, screen } from "@testing-library/react";
import { CommandPalette, type CommandItem } from "../CommandPalette";

function endpointItem(
  id: string,
  method: string,
  path: string
): CommandItem {
  return {
    kind: "endpoint",
    id,
    label: `${method} ${path}`,
    subtitle: "users collection",
    collectionId: "col",
    endpointMethod: method,
    endpointPath: path
  };
}

describe("CommandPalette", () => {
  test("renders nothing when closed", () => {
    const { container } = render(
      <CommandPalette
        open={false}
        items={[]}
        onClose={() => {}}
        onRun={() => {}}
      />
    );
    expect(container.textContent).toBe("");
  });

  test("shows every item when query is empty, then narrows with input", () => {
    const items = [
      endpointItem("a", "GET", "/users"),
      endpointItem("b", "POST", "/orders"),
      endpointItem("c", "GET", "/accounts")
    ];
    render(
      <CommandPalette
        open={true}
        items={items}
        onClose={() => {}}
        onRun={() => {}}
      />
    );
    expect(screen.getAllByRole("option")).toHaveLength(3);

    const input = screen.getByLabelText("Command palette query");
    fireEvent.change(input, { target: { value: "orders" } });
    const results = screen.getAllByRole("option");
    expect(results).toHaveLength(1);
    expect(results[0].textContent).toMatch(/POST \/orders/);
  });

  test("Enter runs the selected item and Esc fires onClose", () => {
    const onRun = vi.fn();
    const onClose = vi.fn();
    const items = [
      endpointItem("a", "GET", "/users"),
      endpointItem("b", "POST", "/orders")
    ];
    render(
      <CommandPalette
        open={true}
        items={items}
        onClose={onClose}
        onRun={onRun}
      />
    );
    const input = screen.getByLabelText("Command palette query");
    // Separate act() calls so React commits the ArrowDown state update
    // before Enter reads the selected index — batching inside one act()
    // leaves Enter looking at the stale initial index 0.
    act(() => {
      fireEvent.keyDown(input, { key: "ArrowDown" });
    });
    act(() => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    expect(onRun).toHaveBeenCalledTimes(1);
    expect(onRun).toHaveBeenCalledWith(items[1]);

    fireEvent.keyDown(input, { key: "Escape" });
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test("arrow-down wraps past the last item", () => {
    const items = [
      endpointItem("a", "GET", "/users"),
      endpointItem("b", "POST", "/orders")
    ];
    const onRun = vi.fn();
    render(
      <CommandPalette
        open={true}
        items={items}
        onClose={() => {}}
        onRun={onRun}
      />
    );
    const input = screen.getByLabelText("Command palette query");
    // 2-item list: start at 0, ArrowDown → 1, ArrowDown → 0 (wrap).
    act(() => {
      fireEvent.keyDown(input, { key: "ArrowDown" });
    });
    act(() => {
      fireEvent.keyDown(input, { key: "ArrowDown" });
    });
    act(() => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    expect(onRun).toHaveBeenCalledWith(items[0]);
  });

  test("empty result set renders a friendly message", () => {
    render(
      <CommandPalette
        open={true}
        items={[endpointItem("a", "GET", "/users")]}
        onClose={() => {}}
        onRun={() => {}}
      />
    );
    const input = screen.getByLabelText("Command palette query");
    fireEvent.change(input, { target: { value: "zzzzz" } });
    expect(screen.getByText(/No matches/i)).toBeTruthy();
  });

  test("running an action calls its run() handler", () => {
    const runAction = vi.fn();
    const action: CommandItem = {
      kind: "action",
      id: "act:toggle",
      label: "Toggle something",
      run: runAction
    };
    const onRun = vi.fn((item: CommandItem) => {
      if (item.kind === "action") item.run();
    });
    render(
      <CommandPalette
        open={true}
        items={[action]}
        onClose={() => {}}
        onRun={onRun}
      />
    );
    fireEvent.click(screen.getByRole("option"));
    expect(onRun).toHaveBeenCalled();
    expect(runAction).toHaveBeenCalled();
  });
});
