import { describe, expect, test, vi } from "vitest";
import { downloadText, timestampSlug } from "../downloadBlob";

describe("timestampSlug", () => {
  test("produces a filesystem-safe timestamp fragment", () => {
    const slug = timestampSlug(new Date("2026-04-24T07:31:05"));
    expect(slug).toBe("2026-04-24T07-31-05");
    expect(slug).not.toMatch(/[:/\\]/);
  });

  test("pads single-digit components", () => {
    const slug = timestampSlug(new Date("2026-01-02T03:04:05"));
    expect(slug).toBe("2026-01-02T03-04-05");
  });
});

describe("downloadText", () => {
  test("creates an anchor with the expected filename and clicks it", () => {
    const createdUrls: string[] = [];
    const originalCreate = URL.createObjectURL;
    const originalRevoke = URL.revokeObjectURL;
    URL.createObjectURL = vi.fn((blob: Blob) => {
      const u = `blob:mock://${blob.size}`;
      createdUrls.push(u);
      return u;
    }) as typeof URL.createObjectURL;
    URL.revokeObjectURL = vi.fn();

    const anchorClick = vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(() => {});

    try {
      downloadText("sample.json", "application/json", "{\"ok\":true}");
      expect(URL.createObjectURL).toHaveBeenCalledTimes(1);
      expect(anchorClick).toHaveBeenCalledTimes(1);
      // The anchor should already be detached after the synchronous path.
      expect(
        document.querySelectorAll("a[download='sample.json']")
      ).toHaveLength(0);
    } finally {
      URL.createObjectURL = originalCreate;
      URL.revokeObjectURL = originalRevoke;
      anchorClick.mockRestore();
    }
  });
});
