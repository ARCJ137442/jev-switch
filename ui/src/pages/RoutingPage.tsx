import { useEffect, useRef, useState } from 'react';
import { getBase } from '../api';
import { findCyclicEdgeKeys, listProviders, listRoutes, normalizeRoute, putRoutes, type Route } from '../api/admin';
import { listEndpoints } from '../api/endpoints';
import { DagCanvas } from '../components/routing/DagCanvas';
import { routeIdentity, routeDocumentSignature, type DagEntry, type DagProvider, type Point } from '../components/routing/dag';
import { DocumentHistory } from '../components/routing/documentHistory';
import { RouteTableForm } from '../components/routing/RouteTableForm';
import { EndpointPanel } from '../components/routing/EndpointPanel';
import { useToast } from '../app/feedback';
import { useI18n } from '../i18n';

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
  const [tab, setTab] = useState<'entries' | 'graph'>('entries');
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
  const [showTable, setShowTable] = useState(false);
  const alive = useRef(true);
  const loadGeneration = useRef(0);
  const inFlight = useRef(false);
  const dirty = serialize(doc.routes) !== serialize(baseline);
  const cycleEdges = findCyclicEdgeKeys(doc.routes);
  const errors = new Set(cycleEdges);

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
      setSelected(null); setSaveError(restored ? t('dag.draftRestored') : null); setSaveFailed(restored);
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
    if (tab !== 'graph') return;
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
  }, [selected, tab]);

  return <div className="w-full min-w-0 px-4 py-4 sm:px-6 lg:px-8">
    <h1 className="mb-5 font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>{t('shell.navRouting')}</h1>
    <div className="routing-tabs" role="tablist" aria-label={t('shell.navRouting')}>
      <button type="button" role="tab" id="entry-tab" aria-controls="entry-panel" aria-selected={tab === 'entries'} onClick={() => setTab('entries')}>{t('entry.tab')}</button>
      <button type="button" role="tab" id="graph-tab" aria-controls="graph-panel" aria-selected={tab === 'graph'} onClick={() => setTab('graph')}>{t('entry.graph')}</button>
    </div>
    {tab === 'entries' ? <div role="tabpanel" id="entry-panel" aria-labelledby="entry-tab"><EndpointPanel disabled={dirty || saving} onChanged={() => void load()} /></div>
      : <div role="tabpanel" id="graph-panel" aria-labelledby="graph-tab">
        <div className="dag-tools">
          <span aria-live="polite">{saving ? t('common.saving') : dirty ? t('routing.unsaved') : t('routing.synced')}</span>
          <span className="dag-help">{t('routing.edges', { n: doc.routes.length })}</span>
          <button type="button" disabled={!history.current.canUndo || loading} onClick={undo} title="Ctrl+Z">{t('dag.undo')}</button>
          <button type="button" disabled={!history.current.canRedo || loading} onClick={redo} title="Ctrl+Shift+Z">{t('dag.redo')}</button>
          <button type="button" aria-pressed={showTable} onClick={() => setShowTable(v => !v)}>{t('routing.table')}</button>
          <button type="button" disabled={dirty || saving || loading} onClick={() => void load()}>{t('common.refresh')}</button>
          {dirty && !saving && <button type="button" onClick={() => { changeRoutes(baseline); setSelected(null); }}>{t('dag.discard')}</button>}
        </div>
        {saveError && <div className="dag-tools" role="alert" style={{ color: 'var(--danger)' }}><span>{saveError}</span>{saveFailed && <button type="button" onClick={() => setSaveFailed(false)}>{t('common.retry')}</button>}</div>}
        {loading ? <p className="py-8" role="status">{t('entry.loading')}</p> : loadError ? <div className="dag-tools" role="alert"><span>{loadError}</span><button type="button" onClick={() => void load()}>{t('common.retry')}</button></div> : <>
          <DagCanvas routes={doc.routes} entries={entries} providers={providers} positions={doc.positions} selected={selected} errors={errors}
            onSelect={setSelected} onCreate={createEdge}
            onPatch={(key, patch) => patchAt(history.current.current.routes.findIndex(r => routeIdentity(r) === key), patch)}
            onDelete={key => removeAt(history.current.current.routes.findIndex(r => routeIdentity(r) === key))}
            onDeleteNode={id => { changeRoutes(history.current.current.routes.filter(r => r.left !== id && r.right !== id)); setSelected(null); }}
            onInsertNode={insertNode}
            onMove={(id, position) => commit({ ...history.current.current, positions: { ...history.current.current.positions, [id]: position } })}
            onResetLayout={() => commit({ ...history.current.current, positions: {} })} />
          {showTable && <div className="mt-4"><RouteTableForm routes={doc.routes} errorEdges={errors} onPatchAt={patchAt} onDeleteAt={removeAt} onAdd={() => changeRoutes([...history.current.current.routes, normalizeRoute({ left: entries[0]?.id ?? '', right: providers[0]?.id ?? '', match: 'exact', priority: 10 })])} /></div>}
        </>}
      </div>}
  </div>;
}
