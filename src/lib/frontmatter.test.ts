import { describe, expect, it } from "vitest";
import { splitFrontmatter } from "./frontmatter";

describe("splitFrontmatter", () => {
  it("separates a leading YAML block from the body", () => {
    expect(
      splitFrontmatter(
        "---\nname: git\nmetadata:\n  type: feedback\n---\n\nBody text\n",
      ),
    ).toEqual({
      frontmatter: "name: git\nmetadata:\n  type: feedback",
      body: "\nBody text\n",
    });
  });

  it("leaves text without frontmatter alone, including later rules", () => {
    const text = "# Title\n\n---\n\nnot: frontmatter\n---\n";

    expect(splitFrontmatter(text)).toEqual({ frontmatter: null, body: text });
  });
});
