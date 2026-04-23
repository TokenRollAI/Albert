import { describe, expect, test } from "vitest";
import { render } from "@testing-library/react";
import { Markdown } from "../Markdown";

describe("Markdown", () => {
  test("renders nothing for empty input", () => {
    const { container } = render(<Markdown source="" />);
    expect(container.textContent).toBe("");
  });

  test("renders paragraphs split on blank lines", () => {
    const { container } = render(
      <Markdown source={"first line\n\nsecond paragraph"} />
    );
    expect(container.querySelectorAll("p.md-p")).toHaveLength(2);
  });

  test("promotes backticks to code spans", () => {
    const { container } = render(<Markdown source="hit `/users/42`" />);
    const code = container.querySelector("code.md-code");
    expect(code?.textContent).toBe("/users/42");
  });

  test("renders bold and italic", () => {
    const { container } = render(
      <Markdown source="**strong** and *subtle*" />
    );
    expect(container.querySelector("strong")?.textContent).toBe("strong");
    expect(container.querySelector("em")?.textContent).toBe("subtle");
  });

  test("renders absolute links with target=_blank", () => {
    const { container } = render(
      <Markdown source="see [docs](https://example.com/api)" />
    );
    const a = container.querySelector("a.md-link");
    expect(a?.getAttribute("href")).toBe("https://example.com/api");
    expect(a?.getAttribute("target")).toBe("_blank");
  });

  test("preserves single newlines as <br>", () => {
    const { container } = render(<Markdown source={"line one\nline two"} />);
    expect(container.querySelectorAll("br")).toHaveLength(1);
  });
});
