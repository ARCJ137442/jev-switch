export function selectExample(
  selectedIds: readonly string[],
  id: string,
  exclusive: boolean,
): string[] {
  if (exclusive) return [id];
  if (!selectedIds.includes(id)) return [...selectedIds, id];
  if (selectedIds.length === 1) return [...selectedIds];
  return selectedIds.filter((selectedId) => selectedId !== id);
}
