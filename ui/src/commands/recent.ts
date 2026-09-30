export const RECENT_COMMANDS_KEY = 'jev-command-history-v1';
export const RECENT_COMMANDS_LIMIT = 20;

export function updateRecentCommands(current: readonly string[], commandId: string): string[] {
  return [commandId, ...current.filter((id) => id !== commandId)].slice(0, RECENT_COMMANDS_LIMIT);
}

export function readRecentCommands(storage?: Pick<Storage, 'getItem'>): string[] {
  try {
    const value: unknown = JSON.parse((storage ?? localStorage).getItem(RECENT_COMMANDS_KEY) ?? '[]');
    return Array.isArray(value) ? value.filter((id): id is string => typeof id === 'string').slice(0, RECENT_COMMANDS_LIMIT) : [];
  } catch {
    return [];
  }
}

export function writeRecentCommands(commandIds: readonly string[], storage?: Pick<Storage, 'setItem'>): void {
  try {
    (storage ?? localStorage).setItem(RECENT_COMMANDS_KEY, JSON.stringify(commandIds.slice(0, RECENT_COMMANDS_LIMIT)));
  } catch {
    // Recent commands are a convenience; the palette remains usable when storage is unavailable.
  }
}
