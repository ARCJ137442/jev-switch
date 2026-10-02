export function settingMatches(query: string, ...content: string[]): boolean {
  const needle = query.trim().toLocaleLowerCase();
  return needle.length === 0 || content.some((text) => text.toLocaleLowerCase().includes(needle));
}

export function highlightSegments(text: string, query: string): { text: string; matched: boolean }[] {
  const needle = query.trim();
  if (!needle) return [{ text, matched: false }];
  const lowerText = text.toLocaleLowerCase();
  const lowerNeedle = needle.toLocaleLowerCase();
  const segments: { text: string; matched: boolean }[] = [];
  let position = 0;
  while (position < text.length) {
    const found = lowerText.indexOf(lowerNeedle, position);
    if (found < 0) { segments.push({ text: text.slice(position), matched: false }); break; }
    if (found > position) segments.push({ text: text.slice(position, found), matched: false });
    segments.push({ text: text.slice(found, found + needle.length), matched: true });
    position = found + needle.length;
  }
  return segments;
}
