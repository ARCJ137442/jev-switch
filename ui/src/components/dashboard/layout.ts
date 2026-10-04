export type DashboardWidth = 'full' | 'wide' | 'half' | 'narrow';

export interface DashboardLayoutItem {
  instanceId: string;
  componentId: string;
  visible: boolean;
  width: DashboardWidth;
  order: number;
}

export interface DashboardLayoutDocument {
  schemaVersion: 1;
  presetId: string;
  revision: number;
  items: DashboardLayoutItem[];
}

export interface DashboardComponentDefinition {
  id: string;
  labelKey: string;
  descriptionKey: string;
  singleton: boolean;
  defaultWidth: DashboardWidth;
  implemented: boolean;
}

export const DASHBOARD_LAYOUT_KEY = 'jev-dashboard-layout-v1';

export const DASHBOARD_COMPONENTS: readonly DashboardComponentDefinition[] = [
  { id: 'runtime', labelKey: 'dashboardLayout.runtime', descriptionKey: 'dashboardLayout.runtimeHint', singleton: true, defaultWidth: 'half', implemented: true },
  { id: 'active-requests', labelKey: 'dashboardLayout.active', descriptionKey: 'dashboardLayout.activeHint', singleton: true, defaultWidth: 'narrow', implemented: true },
  { id: 'providers', labelKey: 'dashboardLayout.providers', descriptionKey: 'dashboardLayout.providersHint', singleton: true, defaultWidth: 'wide', implemented: true },
  { id: 'routes', labelKey: 'dashboardLayout.routes', descriptionKey: 'dashboardLayout.routesHint', singleton: true, defaultWidth: 'narrow', implemented: true },
  { id: 'telemetry', labelKey: 'dashboardLayout.telemetry', descriptionKey: 'dashboardLayout.telemetryHint', singleton: true, defaultWidth: 'full', implemented: true },
  { id: 'quick-actions', labelKey: 'dashboardLayout.actions', descriptionKey: 'dashboardLayout.actionsHint', singleton: true, defaultWidth: 'full', implemented: true },
];

const componentById = new Map(DASHBOARD_COMPONENTS.map((component) => [component.id, component]));

export function createDefaultDashboardLayout(): DashboardLayoutDocument {
  return {
    schemaVersion: 1,
    presetId: 'default',
    revision: 0,
    items: DASHBOARD_COMPONENTS.map((component, order) => ({
      instanceId: `dashboard-${component.id}`,
      componentId: component.id,
      visible: true,
      width: component.defaultWidth,
      order,
    })),
  };
}

function isWidth(value: unknown): value is DashboardWidth {
  return value === 'full' || value === 'wide' || value === 'half' || value === 'narrow';
}

/** Convert imported/old local data into a safe, deterministic dashboard document. */
export function normalizeDashboardLayout(value: unknown, includeDefaults = true): DashboardLayoutDocument {
  const fallback = createDefaultDashboardLayout();
  if (!value || typeof value !== 'object') return fallback;
  const source = value as Partial<DashboardLayoutDocument>;
  const rawItems = Array.isArray(source.items) ? source.items : [];
  const seen = new Set<string>();
  const items: DashboardLayoutItem[] = [];
  for (const raw of rawItems) {
    if (!raw || typeof raw !== 'object') continue;
    const item = raw as Partial<DashboardLayoutItem>;
    if (typeof item.componentId !== 'string' || !componentById.has(item.componentId)) continue;
    const component = componentById.get(item.componentId)!;
    if (component.singleton && seen.has(item.componentId)) continue;
    seen.add(item.componentId);
    items.push({
      instanceId: typeof item.instanceId === 'string' && item.instanceId ? item.instanceId : `dashboard-${item.componentId}`,
      componentId: item.componentId,
      visible: item.visible !== false,
      width: isWidth(item.width) ? item.width : component.defaultWidth,
      order: items.length,
    });
  }
  if (includeDefaults) for (const component of DASHBOARD_COMPONENTS) {
    if (!seen.has(component.id)) items.push({ instanceId: `dashboard-${component.id}`, componentId: component.id, visible: true, width: component.defaultWidth, order: items.length });
  }
  return {
    schemaVersion: 1,
    presetId: typeof source.presetId === 'string' && source.presetId ? source.presetId : 'default',
    revision: typeof source.revision === 'number' && Number.isFinite(source.revision) ? source.revision : 0,
    items,
  };
}

export function readDashboardLayout(storage?: Pick<Storage, 'getItem'>): DashboardLayoutDocument {
  try {
    const raw = (storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage))?.getItem(DASHBOARD_LAYOUT_KEY);
    return raw ? normalizeDashboardLayout(JSON.parse(raw)) : createDefaultDashboardLayout();
  } catch {
    return createDefaultDashboardLayout();
  }
}

export function writeDashboardLayout(layout: DashboardLayoutDocument, storage?: Pick<Storage, 'setItem'>): DashboardLayoutDocument {
  const next = normalizeDashboardLayout({ ...layout, revision: layout.revision + 1 }, false);
  try { (storage ?? (typeof localStorage === 'undefined' ? undefined : localStorage))?.setItem(DASHBOARD_LAYOUT_KEY, JSON.stringify(next)); } catch { /* Session-only fallback. */ }
  return next;
}

export function moveDashboardItem(layout: DashboardLayoutDocument, instanceId: string, direction: -1 | 1): DashboardLayoutDocument {
  const index = layout.items.findIndex((item) => item.instanceId === instanceId);
  const target = index + direction;
  if (index < 0 || target < 0 || target >= layout.items.length) return layout;
  const items = [...layout.items];
  [items[index], items[target]] = [items[target], items[index]];
  return normalizeDashboardLayout({ ...layout, items });
}

export function updateDashboardItem(layout: DashboardLayoutDocument, instanceId: string, patch: Partial<Pick<DashboardLayoutItem, 'visible' | 'width'>>): DashboardLayoutDocument {
  return normalizeDashboardLayout({ ...layout, items: layout.items.map((item) => item.instanceId === instanceId ? { ...item, ...patch } : item) });
}

export function toggleDashboardItem(layout: DashboardLayoutDocument, componentId: string): DashboardLayoutDocument {
  if (layout.items.some((item) => item.componentId === componentId)) return updateDashboardItem(layout, `dashboard-${componentId}`, { visible: !layout.items.find((item) => item.componentId === componentId)!.visible });
  const component = componentById.get(componentId);
  if (!component) return layout;
  return normalizeDashboardLayout({ ...layout, items: [...layout.items, { instanceId: `dashboard-${component.id}`, componentId: component.id, visible: true, width: component.defaultWidth, order: layout.items.length }] });
}

export function addDashboardItem(layout: DashboardLayoutDocument, componentId: string): DashboardLayoutDocument {
  const component = componentById.get(componentId);
  if (!component || layout.items.some((item) => item.componentId === componentId)) return layout;
  return normalizeDashboardLayout({ ...layout, items: [...layout.items, { instanceId: `dashboard-${component.id}`, componentId: component.id, visible: true, width: component.defaultWidth, order: layout.items.length }] });
}

export function removeDashboardItem(layout: DashboardLayoutDocument, instanceId: string): DashboardLayoutDocument {
  return normalizeDashboardLayout({ ...layout, items: layout.items.filter((item) => item.instanceId !== instanceId) }, false);
}
