/**
 * Trigger a browser download of an in-memory string. Used by the Mock
 * Server panel to export the request log. The helper guards the Blob /
 * URL.createObjectURL APIs so it can be imported in unit tests running
 * under jsdom (which supports both).
 */
export function downloadText(
  filename: string,
  mime: string,
  contents: string
): void {
  const blob = new Blob([contents], { type: mime });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.download = filename;
  anchor.rel = "noopener";
  document.body.appendChild(anchor);
  anchor.click();
  anchor.remove();
  // Anchor.click() finishes initiating the download synchronously, so we
  // can release the object URL immediately. Deferring via setTimeout just
  // leaves a dangling handle if the page navigates away before the tick.
  URL.revokeObjectURL(url);
}

/**
 * Returns a filename fragment based on the current local timestamp, e.g.
 * `"2026-04-24T07-31-05"`. Safe for every filesystem (no colons).
 */
export function timestampSlug(now: Date = new Date()): string {
  const pad = (n: number) => String(n).padStart(2, "0");
  const year = now.getFullYear();
  const month = pad(now.getMonth() + 1);
  const day = pad(now.getDate());
  const hour = pad(now.getHours());
  const minute = pad(now.getMinutes());
  const second = pad(now.getSeconds());
  return `${year}-${month}-${day}T${hour}-${minute}-${second}`;
}
