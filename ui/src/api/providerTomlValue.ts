/** The paste form supports quoted strings, booleans and string arrays, never eval. */
export function stripTomlComment(line: string): string {
  let quote = ''; let escaped = false;
  for (let i = 0; i < line.length; i++) {
    const char = line[i];
    if (quote) {
      if (quote === '"' && !escaped && char === '\\') { escaped = true; continue; }
      if (!escaped && char === quote) quote = '';
      escaped = false;
    } else if (char === '"' || char === "'") quote = char;
    else if (char === '#') return line.slice(0, i);
  }
  return line;
}

export function parseProviderTomlValue(raw: string): string | string[] | boolean | null {
  const value = raw.trim();
  if (value === 'true') return true;
  if (value === 'false') return false;
  const parseString = (text: string): string | null => {
    if (text.startsWith("'") && text.endsWith("'") && !text.slice(1, -1).includes("'")) return text.slice(1, -1);
    if (text.startsWith('"') && text.endsWith('"')) {
      try { const parsed: unknown = JSON.parse(text); return typeof parsed === 'string' ? parsed : null; } catch { return null; }
    }
    return null;
  };
  if (!value.startsWith('[')) return parseString(value);
  if (!value.endsWith(']')) return null;
  const body = value.slice(1, -1).trim();
  if (!body) return [];
  let quote = ''; let escaped = false; let start = 0; const parts: string[] = [];
  for (let i = 0; i < body.length; i++) {
    const char = body[i];
    if (quote) {
      if (quote === '"' && !escaped && char === '\\') { escaped = true; continue; }
      if (!escaped && char === quote) quote = '';
      escaped = false;
    } else if (char === '"' || char === "'") quote = char;
    else if (char === ',') { parts.push(body.slice(start, i).trim()); start = i + 1; }
  }
  if (quote) return null;
  const last = body.slice(start).trim(); if (last) parts.push(last);
  const result = parts.map(parseString);
  return result.every((item): item is string => item !== null) ? result : null;
}
