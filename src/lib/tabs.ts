/** Ids that tie a tab bar's tabs to the panel they control. */
export function tabId(idPrefix: string, id: string): string {
  return `${idPrefix}-tab-${id}`;
}

export function panelId(idPrefix: string): string {
  return `${idPrefix}-panel`;
}
