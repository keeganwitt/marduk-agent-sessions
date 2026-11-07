/**
 * Splits a leading `---` YAML block off Markdown. Markdown renderers read it as a rule
 * followed by a heading, so it's shown separately.
 */
export function splitFrontmatter(text: string): {
  frontmatter: string | null;
  body: string;
} {
  const match = /^---\r?\n([\s\S]*?)\r?\n---[ \t]*(?:\r?\n|$)/.exec(text);
  if (!match) return { frontmatter: null, body: text };
  return { frontmatter: match[1], body: text.slice(match[0].length) };
}
