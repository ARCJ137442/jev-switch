import { useEffect, useId, useRef, useState, type FormEvent } from 'react';
import { Check, Copy, Pencil, Plus, Power, RefreshCw, Trash2, X } from 'lucide-react';
import { createEndpoint, deleteEndpoint, listEndpoints, updateDefaultStrategy, updateEndpoint,
  type ServiceEndpointView, type StrategyConfig } from '../../api/endpoints';
import { listProviders, listRoutes, type AdminProvider } from '../../api/admin';
import type { RouteEdge } from '../../generated/RouteEdge';
import { useI18n } from '../../i18n';
import { useToast } from '../../app/feedback';
import './entry.css';

const strategyTypes = ['failover', 'race', 'load_balance', 'shadow'] as const;
const strategyFor = (type: StrategyConfig['type']): StrategyConfig => {
  switch (type) {
    case 'race': return { type, timeout_ms: 5000 };
    case 'load_balance': return { type, weight_mode: 'priority' };
    case 'shadow': return { type, shadow_target: '' };
    default: return { type };
  }
};
const message = (error: unknown) => error instanceof Error ? error.message : String(error);
function storedStrategy(value: string): StrategyConfig | null {
  if (strategyTypes.includes(value as typeof strategyTypes[number])) return strategyFor(value as StrategyConfig['type']);
  try {
    const parsed = JSON.parse(value) as StrategyConfig;
    return parsed && strategyTypes.includes(parsed.type as typeof strategyTypes[number]) ? parsed : null;
  } catch { return null; }
}

interface Props { disabled?: boolean; onChanged: () => void }

export function EndpointPanel({ disabled = false, onChanged }: Props) {
  const { t } = useI18n();
  const { toast } = useToast();
  const [entries, setEntries] = useState<ServiceEndpointView[]>([]);
  const [globalStrategy, setGlobalStrategy] = useState('failover');
  const [editingGlobal, setEditingGlobal] = useState(false);
  const [providers, setProviders] = useState<AdminProvider[]>([]);
  const [routes, setRoutes] = useState<RouteEdge[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [filter, setFilter] = useState('');
  const [editing, setEditing] = useState<ServiceEndpointView | 'new' | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const generation = useRef(0);
  const copyTimer = useRef<ReturnType<typeof setTimeout>>();
  const load = async () => {
    const seq = ++generation.current;
    setLoading(true); setError(null);
    try {
      const [eps, ps, rs] = await Promise.all([listEndpoints(), listProviders(), listRoutes()]);
      if (seq !== generation.current) return;
      setEntries(eps.endpoints); setGlobalStrategy(eps.global_default_strategy); setProviders(ps.providers);
      setRoutes(rs.routes.map(r => ({ ...r, sticky: r.sticky ?? 'none', on_error: r.on_error ?? 'next' })));
    } catch (e) { if (seq === generation.current) setError(message(e)); }
    finally { if (seq === generation.current) setLoading(false); }
  };
  useEffect(() => { void load(); return () => { generation.current++; clearTimeout(copyTimer.current); }; }, []);
  const mutate = async (action: () => Promise<unknown>, success?: string) => {
    if (busy || disabled) return;
    setBusy(true); setError(null);
    try { await action(); if (success) toast('ok', success); await load(); onChanged(); }
    catch (e) { setError(message(e)); }
    finally { setBusy(false); }
  };
  const copy = async (id: string) => {
    try {
      await navigator.clipboard.writeText(id); setCopied(id); clearTimeout(copyTimer.current);
      copyTimer.current = setTimeout(() => setCopied(null), 1800);
    } catch (e) { setError(message(e)); }
  };
  const shown = entries.filter(ep => ep.id.toLowerCase().includes(filter.toLowerCase()));
  const globalConfig = storedStrategy(globalStrategy);
  return <section className="entry-workspace" aria-label={t('entry.tab')}>
    <div className="entry-toolbar">
      <div><h2 className="text-lg font-semibold">{t('entry.tab')}</h2><p className="entry-muted">{t('entry.subtitle')}</p></div>
      <div className="entry-actions">
        <button className="entry-button" disabled={disabled || busy || loading || !globalConfig} onClick={() => setEditingGlobal(true)}>
          <Pencil size={14}/>{t('entry.global')} · {globalConfig ? t(`entry.${globalConfig.type}`) : globalStrategy}
        </button>
        <button className="entry-button" onClick={() => void load()} disabled={busy} aria-label={t('common.refresh')}><RefreshCw size={15}/></button>
        <button className="entry-button primary" onClick={() => setEditing('new')} disabled={disabled || busy || loading}><Plus size={16}/>{t('entry.new')}</button>
      </div>
    </div>
    {disabled && <p className="entry-message entry-muted">{t('entry.saveRoutesFirst')}</p>}
    {error && <div role="alert" className="entry-message entry-error">{error}</div>}
    {loading ? <p role="status" className="entry-message">{t('entry.loading')}</p> : <>
      {entries.length > 0 && <input type="search" className="entry-input" aria-label={t('entry.search')} placeholder={t('entry.search')} value={filter} onChange={e => setFilter(e.target.value)}/>}
      {shown.length === 0 && !error && <p className="entry-message entry-muted">{t(entries.length ? 'entry.noMatches' : 'entry.empty')}</p>}
      <div className="entry-grid">{shown.map(ep => <article key={ep.id} className="entry-card" data-enabled={ep.enabled}>
        <header><h3>{ep.id}</h3><span className="entry-badge" data-enabled={ep.enabled}>{t(ep.enabled ? 'entry.enabled' : 'entry.disabled')}</span></header>
        <div className="entry-actions"><span className="entry-badge">{t(`entry.${ep.strategy_config.type}`)}</span>{ep.routes_count === 0 && <span className="entry-muted">{t('entry.noRoutes')}</span>}</div>
        <div className="entry-stats"><span>{t('entry.routeCount', { n: ep.routes_count })}</span><span>{t('entry.calls', { n: Number(ep.calls_24h) })}</span></div>
        <div className="entry-actions">
          <button className="entry-button" onClick={() => setEditing(ep)} disabled={disabled || busy}><Pencil size={14}/>{t('entry.edit')}</button>
          <button className="entry-button" onClick={() => void copy(ep.id)} title={t(copied === ep.id ? 'entry.copied' : 'entry.copy')} aria-label={t(copied === ep.id ? 'entry.copied' : 'entry.copy')}>{copied === ep.id ? <Check size={14}/> : <Copy size={14}/>}</button>
          <button className="entry-button" title={t(ep.enabled ? 'entry.disable' : 'entry.enable')} aria-label={t(ep.enabled ? 'entry.disable' : 'entry.enable')} disabled={disabled || busy}
            onClick={() => void mutate(() => updateEndpoint(ep.id, { enabled: !ep.enabled }))}><Power size={14}/></button>
          <button className="entry-button danger" aria-label={`${t('common.delete')} ${ep.id}`} disabled={disabled || busy} onClick={() => setDeleting(ep.id)}><Trash2 size={14}/></button>
        </div>
        {deleting === ep.id && <div className="entry-message"><p>{t('entry.deleteTitle', { id: ep.id })}</p><p className="entry-muted">{t('entry.deleteDetail')}</p><div className="entry-actions mt-3">
          <button className="entry-button danger" disabled={busy} onClick={() => void mutate(async () => { await deleteEndpoint(ep.id); setDeleting(null); }, t('entry.deleted'))}>{t('common.delete')}</button>
          <button className="entry-button" disabled={busy} onClick={() => setDeleting(null)}>{t('common.cancel')}</button>
        </div></div>}
      </article>)}</div>
    </>}
    {editing !== null && <EndpointEditor entry={editing === 'new' ? null : editing} providers={providers} allRoutes={routes}
      onClose={() => setEditing(null)} onSaved={() => { setEditing(null); void load(); onChanged(); toast('ok', t('entry.saved')); }}/ >}
    {editingGlobal && globalConfig && <GlobalStrategyEditor initial={globalConfig} providers={providers}
      onClose={() => setEditingGlobal(false)} onSaved={() => { setEditingGlobal(false); void load(); onChanged(); toast('ok', t('entry.globalSaved')); }}/ >}
  </section>;
}

interface EditorProps { entry: ServiceEndpointView | null; providers: AdminProvider[]; allRoutes: RouteEdge[]; onClose: () => void; onSaved: () => void }

function EndpointEditor({ entry, providers, allRoutes, onClose, onSaved }: EditorProps) {
  const { t } = useI18n();
  const prefix = useId();
  const dialog = useRef<HTMLDialogElement>(null);
  const [id, setId] = useState(entry?.id ?? '');
  const [strategy, setStrategy] = useState<StrategyConfig>(entry?.strategy_config ?? { type: 'follow_global' });
  const [rows, setRows] = useState<RouteEdge[]>(allRoutes.filter(r => r.left === entry?.id).map(r => ({ ...r })));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  const patch = (index: number, change: Partial<RouteEdge>) => setRows(current => current.map((r, i) => i === index ? { ...r, ...change } : r));
  const save = async (event: FormEvent) => {
    event.preventDefault(); if (busy) return;
    if (rows.some(r => !r.right.trim())) { setError(t('entry.routeTargetRequired')); return; }
    setBusy(true); setError(null);
    const outgoing = rows.map(r => ({ ...r, left: id.trim(), right: r.right.trim(), upstream_model: r.upstream_model?.trim() || null }));
    try {
      if (entry) await updateEndpoint(entry.id, { id: id.trim(), strategy_config: strategy, routes: outgoing });
      else await createEndpoint({ id: id.trim(), strategy_config: strategy, routes: outgoing });
      onSaved();
    } catch (e) { setError(message(e)); setBusy(false); }
  };
  const targets = [...new Set([...providers.map(p => p.id), ...allRoutes.map(r => r.left)])].filter(target => target !== id);
  return <dialog ref={dialog} className="entry-dialog" aria-labelledby={`${prefix}-title`} onCancel={e => { if (busy) e.preventDefault(); else onClose(); }}>
    <form onSubmit={event => void save(event)}>
      <div className="entry-dialog-header"><h2 id={`${prefix}-title`}>{t(entry ? 'entry.edit' : 'entry.new')}</h2><button type="button" className="entry-button" disabled={busy} onClick={onClose} aria-label={t('common.close')}><X size={16}/></button></div>
      {error && <div className="entry-message entry-error" role="alert">{error}</div>}
      <div className="entry-dialog-body"><fieldset disabled={busy} className="entry-workspace">
        <div className="entry-fields">
          <label className="entry-field">{t('entry.id')}<input className="entry-input mono" autoFocus required maxLength={200} value={id} onChange={e => setId(e.target.value)} placeholder="jev-stable"/><span className="entry-muted">{t('entry.idHelp')}</span></label>
          <label className="entry-field">{t('entry.strategy')}<select className="entry-input" value={strategy.type} onChange={e => setStrategy(strategyFor(e.target.value as StrategyConfig['type']))}>
            <option value="follow_global">{t('entry.follow_global')}</option>{strategyTypes.map(s => <option key={s} value={s}>{t(`entry.${s}`)}</option>)}
          </select></label>
        </div>
        <StrategyParameters strategy={strategy} onChange={setStrategy} providers={providers}/>
        <div className="entry-toolbar"><h3 className="font-semibold">{t('entry.routes')}</h3><button type="button" className="entry-button" onClick={() => setRows([...rows, { left: id, right: providers[0]?.id ?? '', match: 'exact', priority: (rows.length + 1) * 10, sticky: 'none', on_error: 'next', upstream_model: null }])}><Plus size={14}/>{t('entry.addRoute')}</button></div>
        <p className="entry-muted">{t(rows.length ? 'entry.routeHelp' : 'entry.emptyRoutesHelp')}</p>
        <datalist id={`${prefix}-targets`}>{targets.map(target => <option key={target} value={target}/>)}</datalist>
        <div className="entry-route-list">{rows.map((row, index) => <div className="entry-route" key={index}>
          <label className="entry-field">{t('entry.target')}<input className="entry-input" list={`${prefix}-targets`} value={row.right} required onChange={e => patch(index, { right: e.target.value })}/></label>
          <label className="entry-field">{t('entry.upstreamModel')}<input className="entry-input mono" value={row.upstream_model ?? ''} placeholder={t('entry.keepModel')} onChange={e => patch(index, { upstream_model: e.target.value || null })}/></label>
          <label className="entry-field">{t('entry.priority')}<input className="entry-input" type="number" required value={row.priority} onChange={e => patch(index, { priority: Number(e.target.value) })}/></label>
          <label className="entry-field">{t('entry.onError')}<select className="entry-input" value={row.on_error} onChange={e => patch(index, { on_error: e.target.value as RouteEdge['on_error'] })}><option value="next">{t('entry.next')}</option><option value="fail">{t('entry.fail')}</option></select></label>
          <label className="entry-actions entry-muted"><input type="checkbox" checked={row.sticky === 'session'} onChange={e => patch(index, { sticky: e.target.checked ? 'session' : 'none' })}/>{t('entry.sticky')}</label>
          <button className="entry-button danger" type="button" onClick={() => setRows(rows.filter((_, i) => i !== index))} aria-label={`${t('common.remove')} ${index + 1}`}><Trash2 size={14}/>{t('common.remove')}</button>
        </div>)}</div>
      </fieldset></div>
      <footer className="entry-footer"><button type="button" className="entry-button" disabled={busy} onClick={onClose}>{t('common.cancel')}</button><button className="entry-button primary" type="submit" disabled={busy || !id.trim()}>{t(busy ? 'common.saving' : 'common.save')}</button></footer>
    </form>
  </dialog>;
}

function StrategyParameters({ strategy, onChange, providers }: { strategy: StrategyConfig; onChange: (value: StrategyConfig) => void; providers: AdminProvider[] }) {
  const { t } = useI18n();
  const id = useId();
  if (strategy.type === 'race') return <label className="entry-field">{t('entry.timeout')}<input className="entry-input" type="number" min={1} max={300000} required value={Number(strategy.timeout_ms)} onChange={e => onChange({ ...strategy, timeout_ms: Number(e.target.value) })}/></label>;
  if (strategy.type === 'load_balance') return <label className="entry-field">{t('entry.weightMode')}<select className="entry-input" value={strategy.weight_mode} onChange={e => onChange({ ...strategy, weight_mode: e.target.value })}><option value="priority">{t('entry.weightPriority')}</option><option value="equal">{t('entry.weightEqual')}</option></select><span className="entry-muted">{t('entry.weightHelp')}</span></label>;
  if (strategy.type === 'shadow') return <label className="entry-field">{t('entry.shadowTarget')}<input className="entry-input" list={id} value={strategy.shadow_target} onChange={e => onChange({ ...strategy, shadow_target: e.target.value })} required/><datalist id={id}>{providers.filter(provider => provider.enabled).map(provider => <option key={provider.id} value={provider.id}/>)}</datalist></label>;
  return null;
}

function GlobalStrategyEditor({ initial, providers, onClose, onSaved }: { initial: StrategyConfig; providers: AdminProvider[]; onClose: () => void; onSaved: () => void }) {
  const { t } = useI18n();
  const id = useId();
  const dialog = useRef<HTMLDialogElement>(null);
  const [strategy, setStrategy] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => { dialog.current?.showModal(); }, []);
  const save = async (event: FormEvent) => {
    event.preventDefault(); if (busy) return;
    setBusy(true); setError(null);
    try { await updateDefaultStrategy(JSON.stringify(strategy)); onSaved(); }
    catch (error) { setError(message(error)); setBusy(false); }
  };
  return <dialog ref={dialog} className="entry-dialog" aria-labelledby={id} onCancel={event => { if (busy) event.preventDefault(); else onClose(); }}>
    <form onSubmit={event => void save(event)}>
      <div className="entry-dialog-header"><h2 id={id}>{t('entry.global')}</h2><button type="button" className="entry-button" disabled={busy} onClick={onClose} aria-label={t('common.close')}><X size={16}/></button></div>
      <div className="entry-dialog-body"><fieldset disabled={busy} className="entry-workspace">
        <p className="entry-muted">{t('entry.globalHelp')}</p>
        {error && <p className="entry-message entry-error" role="alert">{error}</p>}
        <label className="entry-field">{t('entry.strategy')}<select className="entry-input" value={strategy.type} onChange={event => setStrategy(strategyFor(event.target.value as StrategyConfig['type']))}>{strategyTypes.map(type => <option key={type} value={type}>{t(`entry.${type}`)}</option>)}</select></label>
        <StrategyParameters strategy={strategy} onChange={setStrategy} providers={providers}/>
      </fieldset></div>
      <footer className="entry-footer"><button type="button" className="entry-button" disabled={busy} onClick={onClose}>{t('common.cancel')}</button><button className="entry-button primary" type="submit" disabled={busy}>{t(busy ? 'common.saving' : 'common.save')}</button></footer>
    </form>
  </dialog>;
}
