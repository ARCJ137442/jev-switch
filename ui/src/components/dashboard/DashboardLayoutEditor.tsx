import { useMemo, useState } from 'react';
import { Check, ChevronDown, ChevronUp, Edit3, Plus, RotateCcw, X } from 'lucide-react';
import { useI18n, type MessageKey } from '../../i18n';
import {
  DASHBOARD_COMPONENTS,
  addDashboardItem,
  type DashboardLayoutDocument,
  type DashboardWidth,
  moveDashboardItem,
  removeDashboardItem,
  updateDashboardItem,
  writeDashboardLayout,
} from './layout';

interface Props {
  layout: DashboardLayoutDocument;
  onChange: (layout: DashboardLayoutDocument) => void;
}

const widths: DashboardWidth[] = ['full', 'wide', 'half', 'narrow'];

export function DashboardLayoutEditor({ layout, onChange }: Props) {
  const { t } = useI18n();
  const [editing, setEditing] = useState(() => window.location.hash.includes('edit=1'));
  const [draft, setDraft] = useState(layout);
  const [original, setOriginal] = useState(layout);
  const [past, setPast] = useState<DashboardLayoutDocument[]>([]);
  const [future, setFuture] = useState<DashboardLayoutDocument[]>([]);
  const definitions = useMemo(() => new Map(DASHBOARD_COMPONENTS.map((item) => [item.id, item])), []);
  const begin = () => { setOriginal(layout); setDraft(layout); setPast([]); setFuture([]); setEditing(true); };
  const change = (next: DashboardLayoutDocument) => { setPast((items) => [...items.slice(-19), draft]); setFuture([]); setDraft(next); onChange(next); };
  const cancel = () => { setDraft(original); setPast([]); setFuture([]); onChange(original); setEditing(false); };
  const commit = () => { const saved = writeDashboardLayout(draft); onChange(saved); setEditing(false); };
  const undo = () => { const previous = past.at(-1); if (!previous) return; setFuture((items) => [draft, ...items]); setDraft(previous); onChange(previous); setPast((items) => items.slice(0, -1)); };
  const redo = () => { const next = future[0]; if (!next) return; setPast((items) => [...items, draft]); setDraft(next); onChange(next); setFuture((items) => items.slice(1)); };

  if (!editing) return <button type="button" className="inline-flex items-center gap-2" style={buttonStyle} onClick={begin}><Edit3 size={15} aria-hidden="true" />{t('dashboardLayout.edit' as MessageKey)}</button>;
  return <section className="dashboard-layout-editor" aria-label={t('dashboardLayout.title' as MessageKey)}>
    <div className="dashboard-layout-editor__toolbar">
      <strong>{t('dashboardLayout.title' as MessageKey)}</strong><span className="text-xs" style={{ color: 'var(--text-muted)' }}>{t('dashboardLayout.orderHint' as MessageKey)}</span>
      <span className="dashboard-layout-editor__actions"><button type="button" onClick={undo} disabled={!past.length} style={iconButtonStyle} title={t('dashboardLayout.undo' as MessageKey)}><RotateCcw size={14} /></button><button type="button" onClick={redo} disabled={!future.length} style={iconButtonStyle} title={t('dashboardLayout.redo' as MessageKey)}><RotateCcw size={14} style={{ transform: 'scaleX(-1)' }} /></button><button type="button" onClick={cancel} style={buttonStyle}><X size={14} />{t('common.cancel' as MessageKey)}</button><button type="button" onClick={commit} style={{ ...buttonStyle, borderColor: 'var(--accent)', background: 'var(--accent)', color: '#fff' }}><Check size={14} />{t('dashboardLayout.done' as MessageKey)}</button></span>
    </div>
    <div className="dashboard-layout-editor__list">
      {draft.items.map((item, index) => {
        const definition = definitions.get(item.componentId);
        if (!definition) return null;
        return <div key={item.instanceId} className="dashboard-layout-editor__item" data-visible={item.visible}>
          <button type="button" style={iconButtonStyle} onClick={() => change(moveDashboardItem(draft, item.instanceId, -1))} disabled={index === 0} title={t('dashboardLayout.moveUp' as MessageKey)}><ChevronUp size={15} /></button>
          <button type="button" style={iconButtonStyle} onClick={() => change(moveDashboardItem(draft, item.instanceId, 1))} disabled={index === draft.items.length - 1} title={t('dashboardLayout.moveDown' as MessageKey)}><ChevronDown size={15} /></button>
          <span className="min-w-0 flex-1"><strong>{t(definition.labelKey as MessageKey)}</strong><small>{t(definition.descriptionKey as MessageKey)}</small></span>
          <select aria-label={`${t(definition.labelKey as MessageKey)} ${t('dashboardLayout.widthLabel' as MessageKey)}`} value={item.width} onChange={(event) => change(updateDashboardItem(draft, item.instanceId, { width: event.target.value as DashboardWidth }))} style={selectStyle}>
            {widths.map((width) => <option key={width} value={width}>{t(`dashboardLayout.width.${width}` as MessageKey)}</option>)}
          </select>
          <button type="button" className="dashboard-layout-editor__visibility" aria-pressed={item.visible} onClick={() => change(updateDashboardItem(draft, item.instanceId, { visible: !item.visible }))} style={{ ...buttonStyle, color: item.visible ? 'var(--accent)' : 'var(--text-muted)' }}>{t(item.visible ? 'dashboardLayout.visible' as MessageKey : 'dashboardLayout.hidden' as MessageKey)}</button>
          <button type="button" style={iconButtonStyle} onClick={() => change(removeDashboardItem(draft, item.instanceId))} title={t('dashboardLayout.remove' as MessageKey)} aria-label={`${t('dashboardLayout.remove' as MessageKey)} ${t(definition.labelKey as MessageKey)}`}><X size={14} /></button>
        </div>;
      })}
    </div>
    {DASHBOARD_COMPONENTS.some((component) => !draft.items.some((item) => item.componentId === component.id)) && <div className="flex flex-wrap gap-2">{DASHBOARD_COMPONENTS.filter((component) => !draft.items.some((item) => item.componentId === component.id)).map((component) => <button type="button" key={component.id} style={buttonStyle} onClick={() => change(addDashboardItem(draft, component.id))}><Plus size={14} aria-hidden="true" />{t('dashboardLayout.add' as MessageKey)} {t(component.labelKey as MessageKey)}</button>)}</div>}
  </section>;
}

const buttonStyle: React.CSSProperties = { display: 'inline-flex', alignItems: 'center', gap: '0.35rem', minHeight: '2.25rem', padding: '0.35rem 0.65rem', border: '1px solid var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)', color: 'var(--text)', fontSize: 'var(--text-sm)' };
const iconButtonStyle: React.CSSProperties = { ...buttonStyle, minWidth: '2.25rem', justifyContent: 'center', padding: '0.35rem' };
const selectStyle: React.CSSProperties = { minHeight: '2.25rem', border: '1px solid var(--border)', borderRadius: 'var(--radius)', background: 'var(--surface)', color: 'var(--text)', padding: '0 0.4rem', fontSize: 'var(--text-xs)' };
