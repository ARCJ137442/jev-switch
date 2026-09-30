export function selectExample(
  selectedIds: readonly string[],
  id: string,
  shiftKey: boolean,
): string[] {
  if (!shiftKey) return [id];
  if (!selectedIds.includes(id)) return [...selectedIds, id];
  if (selectedIds.length === 1) return [...selectedIds];
  return selectedIds.filter((selectedId) => selectedId !== id);
}
