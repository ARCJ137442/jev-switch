import { useEffect, useMemo, useRef, useState } from 'react';
import { Redo2, RefreshCw, RotateCcw, Table2, Undo2 } from 'lucide-react';
import { getBase } from '../api';
import { findCyclicEdgeKeys, listProviders, listRoutes, normalizeRoute, putRoutes, type Route } from '../api/admin';
import { listEndpoints } from '../api/endpoints';
import { DagCanvas } from '../components/routing/DagCanvas';
import { routeIdentity, routeDocumentSignature, type DagEntry, type DagProvider, type Point } from '../components/routing/dag';
import { DocumentHistory } from '../components/routing/documentHistory';
import { RouteTableForm } from '../components/routing/RouteTableForm';
import { useToast } from '../app/feedback';
import { useI18n } from '../i18n';
import { useStatusBarItems } from '../app/statusBar';
import { getAdminActivity, streamActivity, type ActivityEvent } from '../api/access';

interface GraphDocument { routes: Route[]; positions: Record<string, Point> }
const serialize = routeDocumentSignature;
const layoutKey = () => `jev-routing-layout-v1:${getBase() || location.origin}`;
const draftKey = () => `jev-routing-draft-v1:${getBase() || location.origin}`;
function readDraft(): Route[] | null {
  try {
    const routes: unknown = JSON.parse(sessionStorage.getItem(draftKey()) ?? 'null');
    if (!Array.isArray(routes) || !routes.every(route => route && typeof route.left === 'string' && typeof route.right === 'string' && Number.isFinite(route.priority))) return null;
    return routes.map(normalizeRoute);
  } catch { return null; }
}
function preserveDraft(routes: Route[], baseline: Route[]) {
  try {
    if (serialize(routes) === serialize(baseline)) sessionStorage.removeItem(draftKey());
    else sessionStorage.setItem(draftKey(), JSON.stringify(routes));
  } catch { /* Unavailable storage does not block editing; beforeunload still warns. */ }
}
function liveEdgesForEvent(routes: Route[], event: ActivityEvent): Map<string, 'success' | 'failure' | 'retry'> {
  const result = new Map<string, 'success' | 'failure' | 'retry'>();
  if (event.kind === 'route_activity') {
    let detail: any;
    try { detail = JSON.parse(event.detail); } catch { return result; }
    const provider = typeof detail?.provider_id === 'string' ? detail.provider_id : null;
    const model = typeof detail?.upstream_model === 'string' ? detail.upstream_model : null;
    const endpoint = typeof detail?.requested_model === 'string' ? detail.requested_model : null;
    const state = detail?.phase === 'finished_success' ? 'success' : detail?.phase === 'finished_failure' ? 'failure' : 'retry';
    for (const route of routes) {
      if ((endpoint && route.left === endpoint && route.right === provider) || (model && route.upstream_model === model)) result.set(routeIdentity(route), state);
    }
    return result;
  }
  if (event.kind !== 'request') return result;
  let detail: any;
  try { detail = JSON.parse(event.detail); } catch { return result; }
  const trace = detail?.route_trace;
  if (!trace || typeof trace !== 'object') return result;
  const status: 'success' | 'failure' = detail.success === true ? 'success' : 'failure';
  const provider = typeof trace.selected_provider === 'string' ? trace.selected_provider : null;
  const selectedModel = typeof trace.selected_model === 'string' ? trace.selected_model : null;
  const endpoint = typeof detail.endpoint_id === 'string' ? detail.endpoint_id : typeof trace.requested_model === 'string' ? trace.requested_model : null;
  for (const route of routes) {
    if (endpoint && route.left === endpoint && (route.right === provider || route.upstream_model === selectedModel)) result.set(routeIdentity(route), status);
  }
  const attempts = Array.isArray(trace.attempts) ? trace.attempts : [];
  for (const attempt of attempts) {
    const attemptProvider = typeof attempt.provider_id === 'string' ? attempt.provider_id : null;
    const outcome = attempt.outcome === 'failed' ? 'failure' : attempt.outcome === 'succeeded' ? 'success' : attempt.retry_decision === 'retry_same_candidate' ? 'retry' : null;
    if (!attemptProvider || !outcome) continue;
    for (const route of routes) {
      if ((endpoint && route.left === endpoint && route.right === attemptProvider) || route.right === attemptProvider || route.upstream_model === attempt.upstream_model) result.set(routeIdentity(route), outcome);
    }
  }
  return result;
}
function readPositions(): Record<string, Point> {
  try {
    const stored = JSON.parse(localStorage.getItem(layoutKey()) ?? '{}') as Record<string, Point>;
    return Object.fromEntries(Object.entries(stored).filter(([, p]) => p && Number.isFinite(p.x) && Number.isFinite(p.y) && p.x >= 0 && p.y >= 0));
  } catch { return {}; }
}
const isEditing = (target: EventTarget | null) => target instanceof HTMLElement && Boolean(target.closest('input,textarea,select,[contenteditable="true"],[role="dialog"]'));

export function RoutingPage() {
  const { t } = useI18n();
  const { toast } = useToast();
  const [doc, setDoc] = useState<GraphDocument>(() => ({ routes: [], positions: readPositions() }));
  const history = useRef(new DocumentHistory(doc));
  const [entries, setEntries] = useState<DagEntry[]>([]);
  const [providers, setProviders] = useState<DagProvider[]>([]);
  const [baseline, setBaseline] = useState<Route[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveFailed, setSaveFailed] = useState(false);
  const [selected, setSelected] = useState<string | null>(null);
  const [selectedNode, setSelectedNode] = useState<string | null>(null);
  const [showTable, setShowTable] = useState(false);
  const [liveEdges, setLiveEdges] = useState<Map<string, 'success' | 'failure' | 'retry'>>(new Map());
  const alive = useRef(true);
  const loadGeneration = useRef(0);
  const inFlight = useRef(false);
  const dirty = serialize(doc.routes) !== serialize(baseline);
  const cycleEdges = findCyclicEdgeKeys(doc.routes);
  const errors = new Set(cycleEdges);
  const statusItems = useMemo(() => [
    { id: 'routing-edges', label: t('routing.edges', { n: doc.routes.length }) },
    { id: 'routing-save-state', label: t(dirty ? 'routing.unsaved' : 'routing.synced'), tone: saveError || loadError ? 'danger' as const : dirty ? 'warning' as const : 'good' as const },
  ], [dirty, doc.routes.length, loadError, saveError, t]);
  useStatusBarItems(statusItems);

  const load = async () => {
    const generation = ++loadGeneration.current;
    setLoading(true); setLoadError(null);
    try {
      const [routeDoc, providerDoc, entryDoc] = await Promise.all([listRoutes(), listProviders(), listEndpoints()]);
      if (!alive.current || generation !== loadGeneration.current) return;
      const routes = routeDoc.routes.map(normalizeRoute);
      const draft = readDraft();
      const restored = draft !== null && serialize(draft) !== serialize(routes);
      setDoc(history.current.reset({ routes: restored ? draft : routes, positions: history.current.current.positions }));
      setBaseline(routes);
      setProviders(providerDoc.providers);
      setEntries(entryDoc.endpoints.map(entry => ({ id: entry.id, enabled: entry.enabled, strategy: entry.strategy_config.type })));
      setSelected(null); setSelectedNode(null); setSaveError(restored ? t('dag.draftRestored') : null); setSaveFailed(restored);
    } catch (error) {
      if (alive.current && generation === loadGeneration.current) setLoadError(error instanceof Error ? error.message : String(error));
    } finally { if (alive.current && generation === loadGeneration.current) setLoading(false); }
  };
  useEffect(() => {
    alive.current = true; void load();
    return () => { alive.current = false; loadGeneration.current++; };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const commit = (next: GraphDocument) => { preserveDraft(next.routes, baseline); setDoc(history.current.push(next)); setSaveError(null); setSaveFailed(false); };
  const changeRoutes = (routes: Route[]) => commit({ ...history.current.current, routes });
  const undo = () => { const next = history.current.undo(); preserveDraft(next.routes, baseline); setDoc(next); setSelected(null); setSaveError(null); setSaveFailed(false); };
  const redo = () => { const next = history.current.redo(); preserveDraft(next.routes, baseline); setDoc(next); setSelected(null); setSaveError(null); setSaveFailed(false); };

  // One PUT at a time: acknowledge its snapshot, then send any newer local edits.
  useEffect(() => {
    if (loading || loadError || !dirty || saving || saveFailed || inFlight.current) return;
    if (cycleEdges.length) { setSaveError(t('routing.cycle')); return; }
    if (doc.routes.some(route => !route.left.trim() || !route.right.trim())) { setSaveError(t('dag.invalid')); return; }
    if (new Set(doc.routes.map(routeIdentity)).size !== doc.routes.length) { setSaveError(t('dag.duplicate')); return; }
    const current = doc.routes.map(normalizeRoute);
    const timer = window.setTimeout(async () => {
      if (inFlight.current) return;
      inFlight.current = true; setSaving(true); setSaveError(null);
      try {
        await putRoutes(current);
        // A PUT body may merely echo the request; independently read the stored graph.
        const result = await listRoutes();
        const acknowledged = result.routes.map(normalizeRoute);
        if (serialize(current) !== serialize(acknowledged)) throw new Error(t('dag.saveMismatch'));
        if (alive.current) setBaseline(acknowledged);
      } catch (error) {
        if (alive.current) { setSaveFailed(true); setSaveError(error instanceof Error ? error.message : String(error)); }
      } finally { inFlight.current = false; if (alive.current) setSaving(false); }
    }, 400);
    return () => window.clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [doc.routes, baseline, loading, loadError, dirty, saving, saveFailed]);

  useEffect(() => { try { localStorage.setItem(layoutKey(), JSON.stringify(doc.positions)); } catch { /* Keep layout in memory. */ } }, [doc.positions]);
  useEffect(() => { if (!loading && !loadError) preserveDraft(doc.routes, baseline); }, [doc.routes, baseline, loading, loadError]);
  useEffect(() => {
    if (loading || loadError) return;
    const controller = new AbortController();
    let active = true;
    void getAdminActivity(0, 1).then((page) => streamActivity({ type: 'admin' }, page.next_since, controller.signal, (event) => {
      if (!active) return;
      const next = liveEdgesForEvent(doc.routes, event);
      if (!next.size) return;
      setLiveEdges(next);
      window.setTimeout(() => { if (active) setLiveEdges(new Map()); }, 3200);
    })).catch(() => undefined);
    return () => { active = false; controller.abort(); };
  }, [doc.routes, loading, loadError]);
  useEffect(() => {
    if (!dirty && !saving) return;
    const warn = (event: BeforeUnloadEvent) => { event.preventDefault(); event.returnValue = ''; };
    window.addEventListener('beforeunload', warn);
    return () => window.removeEventListener('beforeunload', warn);
  }, [dirty, saving]);

  const patchAt = (index: number, patch: Partial<Route>) => {
    const current = history.current.current.routes;
    if (!current[index]) return;
    const route = normalizeRoute({ ...current[index], ...patch });
    if (routeIdentity(current[index]) === selected) setSelected(routeIdentity(route));
    changeRoutes(current.map((old, i) => i === index ? route : old));
  };

  const openNode = (node: { id: string; kind: 'entry' | 'provider' | 'alias' }) => {
    if (node.kind === 'entry') window.location.hash = `#/endpoints?focus=${encodeURIComponent(node.id)}`;
    else if (node.kind === 'provider') window.location.hash = `#/providers?focus=${encodeURIComponent(node.id)}`;
  };
  const removeAt = (index: number) => {
    changeRoutes(history.current.current.routes.filter((_, i) => i !== index)); setSelected(null);
  };
  const createEdge = (left: string, right: string, model?: string | null) => {
    const routes = history.current.current.routes;
    const route = normalizeRoute({ left, right, match: left.endsWith('*') ? 'prefix' : 'exact', upstream_model: model ?? undefined, priority: Math.max(0, ...routes.filter(r => r.left === left).map(r => r.priority)) + 10 });
    const key = routeIdentity(route);
    if (routes.some(r => routeIdentity(r) === key)) { setSelected(key); toast('warn', t('dag.duplicate')); return; }
    changeRoutes([...routes, route]); setSelected(key);
  };
  const insertNode = (key: string, id: string) => {
    const routes = history.current.current.routes;
    if (!id || [...entries, ...providers].some(item => item.id === id) || routes.some(r => r.left === id || r.right === id)) { toast('warn', t('dag.idExists')); return; }
    const index = routes.findIndex(r => routeIdentity(r) === key);
    if (index < 0) return;
    const original = routes[index];
    const first = normalizeRoute({ ...original, right: id, upstream_model: undefined });
    const second = normalizeRoute({ ...original, left: id, match: 'exact', priority: 10 });
    changeRoutes([...routes.slice(0, index), first, second, ...routes.slice(index + 1)]);
    setSelected(routeIdentity(second));
  };

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (isEditing(event.target)) return;
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'z') { event.preventDefault(); if (event.shiftKey) redo(); else undo(); }
      else if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'y') { event.preventDefault(); redo(); }
      else if ((event.key === 'Delete' || event.key === 'Backspace') && selected) {
        event.preventDefault(); const index = history.current.current.routes.findIndex(r => routeIdentity(r) === selected); if (index >= 0) removeAt(index);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected]);

  return <div className="routing-page">
    <div className="routing-page__status" aria-live="polite">
      <span className="inline-flex items-center gap-2"><span className={`h-2 w-2 rounded-full ${saving ? 'animate-pulse' : ''}`} style={{ background: saveError || loadError ? 'var(--danger)' : dirty ? 'var(--warning)' : 'var(--success)' }}/>{saving ? t('common.saving') : dirty ? t('routing.unsaved') : t('routing.synced')}<span style={{ color: 'var(--text-subtle)' }}>{t('routing.edges', { n: doc.routes.length })}</span></span>
      <button className="dag-icon-button" type="button" disabled={!history.current.canUndo || loading} onClick={undo} title={`${t('dag.undo')} (Ctrl+Z)`} aria-label={t('dag.undo')}><Undo2 size={16} aria-hidden="true" /></button>
      <button className="dag-icon-button" type="button" disabled={!history.current.canRedo || loading} onClick={redo} title={`${t('dag.redo')} (Ctrl+Shift+Z)`} aria-label={t('dag.redo')}><Redo2 size={16} aria-hidden="true" /></button>
      <button className="dag-icon-button" type="button" aria-pressed={showTable} onClick={() => setShowTable(v => !v)} title={t('routing.table')} aria-label={t('routing.table')}><Table2 size={16} aria-hidden="true" /></button>
      <button className="dag-icon-button" type="button" disabled={dirty || saving || loading} onClick={() => void load()} title={t('common.refresh')} aria-label={t('common.refresh')}><RefreshCw size={15} aria-hidden="true" /></button>
      {dirty && !saving && <button className="dag-icon-button" type="button" onClick={() => { changeRoutes(baseline); setSelected(null); }} title={t('dag.discard')} aria-label={t('dag.discard')}><RotateCcw size={16} aria-hidden="true" /></button>}
    </div>
    {(saveError || loadError) && <div className="routing-page__error" role="alert"><span>{saveError ?? loadError}</span>{saveFailed && <button type="button" onClick={() => setSaveFailed(false)}>{t('common.retry')}</button>}</div>}
    {loading ? <p className="routing-page__loading" role="status">{t('entry.loading')}</p> : loadError ? <button className="routing-page__loading" type="button" onClick={() => void load()}>{t('common.retry')}</button> : <DagCanvas routes={doc.routes} entries={entries} providers={providers} positions={doc.positions} selected={selected} selectedNode={selectedNode} errors={errors} liveEdges={liveEdges}
      onSelectNode={setSelectedNode} onNodeDoubleClick={openNode}
      onSelect={setSelected} onCreate={createEdge}
      onPatch={(key, patch) => patchAt(history.current.current.routes.findIndex(r => routeIdentity(r) === key), patch)}
      onDelete={key => removeAt(history.current.current.routes.findIndex(r => routeIdentity(r) === key))}
      onDeleteNode={id => { changeRoutes(history.current.current.routes.filter(r => r.left !== id && r.right !== id)); setSelected(null); }}
      onInsertNode={insertNode}
      onMove={(id, position) => commit({ ...history.current.current, positions: { ...history.current.current.positions, [id]: position } })}
      onResetLayout={() => commit({ ...history.current.current, positions: {} })} />}
    {showTable && <aside className="routing-page__table" aria-label={t('routing.table')}><header><strong>{t('routing.table')}</strong><button className="dag-icon-button" type="button" onClick={() => setShowTable(false)} aria-label={t('common.close')} title={t('common.close')}>×</button></header><div className="routing-page__table-body"><RouteTableForm routes={doc.routes} errorEdges={errors} onPatchAt={patchAt} onDeleteAt={removeAt} onAdd={() => changeRoutes([...history.current.current.routes, normalizeRoute({ left: entries[0]?.id ?? '', right: providers[0]?.id ?? '', match: 'exact', priority: 10 })])} /></div></aside>}
  </div>;
}
