import { afterEach, describe, expect, test, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { ScenariosPanel } from "../ScenariosPanel";
import type { StoredScenarioSummary } from "../../types";

function fixture(name: string, suffix = "1"): StoredScenarioSummary {
  return {
    id: `scenario-${suffix}`,
    name,
    created_at: "1700000000",
    updated_at: "1700000001"
  };
}

afterEach(() => {
  vi.restoreAllMocks();
});

describe("ScenariosPanel", () => {
  test("renders the initial list from listScenarios", async () => {
    const listScenarios = vi
      .fn()
      .mockResolvedValue([fixture("alpha", "a"), fixture("beta", "b")]);
    render(
      <ScenariosPanel
        running={true}
        listScenarios={listScenarios}
        onSave={vi.fn()}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onRename={vi.fn()}
      />
    );
    await waitFor(() => expect(listScenarios).toHaveBeenCalled());
    expect(await screen.findByText("alpha")).toBeDefined();
    expect(screen.getByText("beta")).toBeDefined();
  });

  test("Save button disabled while name is blank", async () => {
    render(
      <ScenariosPanel
        running={true}
        listScenarios={vi.fn().mockResolvedValue([])}
        onSave={vi.fn()}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onRename={vi.fn()}
      />
    );
    const save = screen.getAllByRole("button", { name: /Save/ })[0];
    expect((save as HTMLButtonElement).disabled).toBe(true);
  });

  test("typing a name and clicking Save invokes onSave then re-lists", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    const listScenarios = vi
      .fn()
      .mockResolvedValueOnce([]) // initial load
      .mockResolvedValueOnce([fixture("broken")]); // after save
    render(
      <ScenariosPanel
        running={true}
        listScenarios={listScenarios}
        onSave={onSave}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onRename={vi.fn()}
      />
    );
    await waitFor(() => expect(listScenarios).toHaveBeenCalledTimes(1));

    const input = screen.getByRole("textbox") as HTMLInputElement;
    fireEvent.change(input, { target: { value: "broken" } });
    const [saveBtn] = screen.getAllByRole("button", { name: /Save/ });
    fireEvent.click(saveBtn);

    await waitFor(() => expect(onSave).toHaveBeenCalledWith("broken"));
    await waitFor(() => expect(listScenarios).toHaveBeenCalledTimes(2));
    expect(await screen.findByText("broken")).toBeDefined();
    expect(input.value).toBe("");
  });

  test("Load button disabled when gateway is not running", async () => {
    render(
      <ScenariosPanel
        running={false}
        listScenarios={vi.fn().mockResolvedValue([fixture("alpha")])}
        onSave={vi.fn()}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onRename={vi.fn()}
      />
    );
    const load = await screen.findByRole("button", { name: /Load/ });
    expect((load as HTMLButtonElement).disabled).toBe(true);
  });

  test("Delete triggers onDelete and refreshes the list", async () => {
    const onDelete = vi.fn().mockResolvedValue(undefined);
    const listScenarios = vi
      .fn()
      .mockResolvedValueOnce([fixture("alpha")])
      .mockResolvedValueOnce([]);
    render(
      <ScenariosPanel
        running={true}
        listScenarios={listScenarios}
        onSave={vi.fn()}
        onLoad={vi.fn()}
        onDelete={onDelete}
        onRename={vi.fn()}
      />
    );
    const del = await screen.findByRole("button", { name: "Delete" });
    fireEvent.click(del);
    await waitFor(() => expect(onDelete).toHaveBeenCalledWith("alpha"));
    await waitFor(() => expect(listScenarios).toHaveBeenCalledTimes(2));
  });

  test("Rename swaps the row into an input and commits on Enter", async () => {
    const onRename = vi.fn().mockResolvedValue(undefined);
    const listScenarios = vi
      .fn()
      .mockResolvedValueOnce([fixture("alpha")])
      .mockResolvedValueOnce([fixture("final")]);
    render(
      <ScenariosPanel
        running={true}
        listScenarios={listScenarios}
        onSave={vi.fn()}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onRename={onRename}
      />
    );
    fireEvent.click(await screen.findByRole("button", { name: "Rename" }));
    const inputs = screen.getAllByRole("textbox");
    // Two inputs are visible: the draft "Save current as" at top, and the rename field.
    const renameInput = inputs.find(
      (el) => (el as HTMLInputElement).value === "alpha"
    ) as HTMLInputElement;
    fireEvent.change(renameInput, { target: { value: "final" } });
    fireEvent.keyDown(renameInput, { key: "Enter" });
    await waitFor(() =>
      expect(onRename).toHaveBeenCalledWith("alpha", "final")
    );
  });

  test("rename Escape aborts without calling onRename", async () => {
    const onRename = vi.fn();
    render(
      <ScenariosPanel
        running={true}
        listScenarios={vi.fn().mockResolvedValue([fixture("alpha")])}
        onSave={vi.fn()}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onRename={onRename}
      />
    );
    fireEvent.click(await screen.findByRole("button", { name: "Rename" }));
    const renameInput = screen
      .getAllByRole("textbox")
      .find(
        (el) => (el as HTMLInputElement).value === "alpha"
      ) as HTMLInputElement;
    fireEvent.change(renameInput, { target: { value: "final" } });
    fireEvent.keyDown(renameInput, { key: "Escape" });
    expect(onRename).not.toHaveBeenCalled();
  });

  test("renaming to the same name is a no-op (doesn't call backend)", async () => {
    const onRename = vi.fn();
    render(
      <ScenariosPanel
        running={true}
        listScenarios={vi.fn().mockResolvedValue([fixture("alpha")])}
        onSave={vi.fn()}
        onLoad={vi.fn()}
        onDelete={vi.fn()}
        onRename={onRename}
      />
    );
    fireEvent.click(await screen.findByRole("button", { name: "Rename" }));
    const renameInput = screen
      .getAllByRole("textbox")
      .find(
        (el) => (el as HTMLInputElement).value === "alpha"
      ) as HTMLInputElement;
    fireEvent.keyDown(renameInput, { key: "Enter" });
    expect(onRename).not.toHaveBeenCalled();
  });
});
