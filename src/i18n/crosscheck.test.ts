import { describe, expect, it } from "vitest";
import { LOCALES } from "./locales";
import { messagesFor } from "./messages";

describe("cross-references between catalogs", () => {
  it("names buttons the way the buttons themselves are translated", () => {
    for (const { id } of LOCALES) {
      const messages = messagesFor(id);
      const button = String(messages["app.labels"]);
      const hint = Object.entries(messages).find(
        ([key, text]) =>
          key.startsWith("settings.") && String(text).includes("▾"),
      );
      expect(hint, id).toBeDefined();
      expect(String(hint![1]), id).toContain(button);
    }
  });
});
