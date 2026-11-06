const ALLOWED_PROTOCOLS = new Set(["http:", "https:", "mailto:"]);

export function externalUrl(href: string | null): string | null {
  if (!href) return null;
  try {
    const url = new URL(href);
    return ALLOWED_PROTOCOLS.has(url.protocol) ? url.href : null;
  } catch {
    return null;
  }
}

/**
 * Routes every link click to the OS browser. Tauri's webview does not open
 * `target="_blank"` links by itself, so without this, links silently do nothing.
 */
export function installExternalLinkHandler(
  root: Pick<Document, "addEventListener" | "removeEventListener">,
  open: (url: string) => Promise<void> | void,
): () => void {
  const onClick = (event: MouseEvent) => {
    const target = event.target;
    const anchor = target instanceof Element ? target.closest("a[href]") : null;
    if (!anchor) return;
    // Never let the webview itself navigate away from the app.
    event.preventDefault();
    const url = externalUrl(anchor.getAttribute("href"));
    if (url) void open(url);
  };
  root.addEventListener("click", onClick);
  return () => root.removeEventListener("click", onClick);
}
