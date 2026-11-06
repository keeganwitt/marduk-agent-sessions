/** Splits a path into its folder and final name, for "/" and "\" separators. */
export function splitPath(path: string): { dir: string; name: string } {
  const index = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (index < 0) return { dir: "", name: path };
  return { dir: path.slice(0, index), name: path.slice(index + 1) };
}

/** The folder every path is directly inside, or null when they differ. */
export function sharedFolder(paths: readonly string[]): string | null {
  if (paths.length === 0) return null;
  const first = splitPath(paths[0]).dir;
  return paths.every((path) => splitPath(path).dir === first) ? first : null;
}
