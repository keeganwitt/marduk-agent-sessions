import { describe, expect, it, vi } from "vitest";
import { externalUrl, installExternalLinkHandler } from "./externalLinks";

describe("externalUrl", () => {
  it.each([
    ["https://example.com/a?b=1", "https://example.com/a?b=1"],
    ["http://example.com", "http://example.com/"],
    ["mailto:me@example.com", "mailto:me@example.com"],
  ])("allows %s", (href, expected) => {
    expect(externalUrl(href)).toBe(expected);
  });

  it.each([
    "javascript:alert(1)",
    "file:///etc/passwd",
    "tauri://localhost",
    "#section",
    "relative/path",
    "",
    null,
  ])("rejects %s", (href) => {
    expect(externalUrl(href)).toBeNull();
  });
});

describe("installExternalLinkHandler", () => {
  function clickLink(href: string, inner = false) {
    document.body.innerHTML = `<a href="${href}"><span>label</span></a>`;
    const target = inner
      ? document.querySelector("span")!
      : document.querySelector("a")!;
    const event = new MouseEvent("click", { bubbles: true, cancelable: true });
    target.dispatchEvent(event);
    return event;
  }

  it("opens allowed links externally and blocks webview navigation", () => {
    const open = vi.fn();
    const uninstall = installExternalLinkHandler(document, open);

    const event = clickLink("https://example.com", true);

    expect(open).toHaveBeenCalledWith("https://example.com/");
    expect(event.defaultPrevented).toBe(true);
    uninstall();
  });

  it("blocks disallowed schemes without opening them", () => {
    const open = vi.fn();
    const uninstall = installExternalLinkHandler(document, open);

    const event = clickLink("javascript:alert(1)");

    expect(open).not.toHaveBeenCalled();
    expect(event.defaultPrevented).toBe(true);
    uninstall();
  });

  it("ignores clicks outside links and stops after uninstall", () => {
    const open = vi.fn();
    const uninstall = installExternalLinkHandler(document, open);
    document.body.innerHTML = "<button>b</button>";
    document.querySelector("button")!.click();
    uninstall();
    const blockJsdomNavigation = (event: Event) => event.preventDefault();
    document.addEventListener("click", blockJsdomNavigation);
    clickLink("https://example.com");
    document.removeEventListener("click", blockJsdomNavigation);

    expect(open).not.toHaveBeenCalled();
  });
});
