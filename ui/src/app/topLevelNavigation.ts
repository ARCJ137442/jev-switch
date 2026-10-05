export const TOP_LEVEL_ROUTES = [
  'dashboard',
  'providers',
  'endpoints',
  'routing',
  'playground',
  'stats',
  'settings',
] as const;

export type TopLevelRoute = (typeof TOP_LEVEL_ROUTES)[number];

export interface TopLevelShortcutInput {
  key: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
  isComposing: boolean;
  defaultPrevented: boolean;
}

/** Ctrl+Tab belongs to the Tauri app; browser tabs retain the shortcut elsewhere. */
export function isTopLevelShortcut(input: TopLevelShortcutInput, inTauri: boolean): boolean {
  return inTauri
    && input.key === 'Tab'
    && (input.ctrlKey || input.metaKey)
    && !input.altKey
    && !input.isComposing
    && !input.defaultPrevented;
}

/** Editable controls keep native focus and text editing shortcuts. */
export function isEditableShortcutTarget(target: unknown): boolean {
  if (!target || typeof target !== 'object') return false;
  const element = target as {
    tagName?: string;
    isContentEditable?: boolean;
    closest?: (selector: string) => unknown;
  };
  if (element.isContentEditable) return true;
  const tag = element.tagName?.toLowerCase();
  if (tag === 'input' || tag === 'textarea' || tag === 'select') return true;
  return typeof element.closest === 'function'
    && Boolean(element.closest('input,textarea,select,[contenteditable="true"],[contenteditable=""]'));
}

/** Cycle top-level pages like browser tabs, preserving the requested direction. */
export function cycleTopLevelRoute(
  current: string,
  reverse = false,
  routes: readonly TopLevelRoute[] = TOP_LEVEL_ROUTES,
): TopLevelRoute {
  const available = routes.length > 0 ? routes : TOP_LEVEL_ROUTES;
  const currentIndex = current === 'home' ? 0 : available.indexOf(current as TopLevelRoute);
  const index = currentIndex < 0 ? 0 : currentIndex;
  const offset = reverse ? -1 : 1;
  return available[(index + offset + available.length) % available.length];
}
