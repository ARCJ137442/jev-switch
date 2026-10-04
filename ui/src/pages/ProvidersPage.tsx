import { useEffect, useMemo, useRef, useState } from 'react';
import { Search } from 'lucide-react';
import {
  getAdminMode,
  listProviders,
  mutateMockExternally,
  putProviders,
  type AdminProvider,
  type AdminProviderWrite,
  type ProvidersResponse,
} from '../api/admin';
import { useConfigConflict } from '../hooks/useConfigConflict';
import { useStatusBarItems } from '../app/statusBar';
import { useToast } from '../app/feedback';
import { ProviderCard } from '../components/providers/ProviderCard';
import { AddProviderPanel } from '../components/providers/AddProviderPanel';
import { ConfigFileSync } from '../components/providers/ConfigFileSync';
import { useI18n, type MessageKey } from '../i18n';
import { getAdminActivity } from '../api/access';
import { summarizeProviderAvailability } from './providerAvailability';

const toWrite = (p: AdminProvider): AdminProviderWrite => ({
  id: p.id,
  name: p.name,
  account: p.account,
  kind: p.kind,
  base: p.base,
  enabled: p.enabled,
  models: p.models ? [...p.models] : undefined,
});

/**
 * Providers 页（v2 设计系统 · docs/design/UI-REDESIGN-v2.md §3.2，契约 06 §3）：
 * 卡片流 + ENABLED 乐观更新/回滚 + 密文密钥 + Probe + 普通二次确认删除 +
 * 表单/贴 toml 双入口 + 冲突横幅（GET 深比较驱动，hook 集中供 H3 复用）。
 */
export function ProvidersPage() {
  const [providers, setProviders] = useState<AdminProvider[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [filter, setFilter] = useState(() => new URLSearchParams(window.location.hash.split('?')[1] ?? '').get('focus') ?? '');
  const [availabilityEvents, setAvailabilityEvents] = useState<Awaited<ReturnType<typeof getAdminActivity>>['events']>([]);
  const [addTab, setAddTab] = useState<'form' | 'toml'>('form');
  const { toast } = useToast();
  const { t } = useI18n();

  const providersRef = useRef(providers);
  providersRef.current = providers;

  const { sync, noteBaseline } = useConfigConflict<ProvidersResponse>({
    fetchSnapshot: listProviders,
    applySnapshot: (snap) => setProviders(snap.providers),
    getCurrent: () => ({ providers: providersRef.current.map((p) => ({ ...p })) }),
    pushSnapshot: async (snap) => {
      await putProviders(snap.providers.map(toWrite));
    },
    pollMs: 5000,
  });

  const load = () => {
    setLoading(true);
    setError(null);
    sync()
      .then(() => setLoading(false))
      .catch((e) => {
        setError((e as Error).message);
        setLoading(false);
      });
  };

  useEffect(() => {
    load();
    getAdminActivity(0, 50).then((result) => setAvailabilityEvents(result.events)).catch(() => setAvailabilityEvents([]));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // dev：验证冲突横幅 —— 控制台执行 __jevMockExternalMutate() 模拟外部改 toml
  useEffect(() => {
    if (import.meta.env.DEV) {
      (window as unknown as { __jevMockExternalMutate?: () => void }).__jevMockExternalMutate =
        mutateMockExternally;
    }
  }, []);

  const commit = async (writes: AdminProviderWrite[]) => {
    const res = await putProviders(writes);
    setProviders(res.providers);
    noteBaseline(res);
    return res;
  };

  /* ENABLED 开关 — 乐观更新，失败回滚 + toast */
  const onToggle = async (p: AdminProvider, enabled: boolean) => {
    const prev = providersRef.current;
    const optimistic = prev.map((x) => (x.id === p.id ? { ...x, enabled } : x));
    setBusyId(p.id);
    setProviders(optimistic); // 即时反馈
    try {
      await commit(optimistic.map(toWrite));
    } catch {
      setProviders(prev); // 回滚
      toast('danger', t('prov.saveFailed'));
    } finally {
      setBusyId(null);
    }
  };

  /* Replace key — 响应只回 masked；成功 toast 契约 04 文案 */
  const onReplaceKey = async (p: AdminProvider, apiKey: string) => {
    setBusyId(p.id);
    try {
      const writes = providersRef.current.map((x) => {
        const w = toWrite(x);
        if (x.id === p.id) w.api_key = apiKey;
        return w;
      });
      await commit(writes);
      toast('ok', t('prov.keySaved'));
    } catch (e) {
      toast('danger', t('prov.keySaveFailed'));
      throw e; // KeyForm 保持打开
    } finally {
      setBusyId(null);
    }
  };

  const onClearKey = async (p: AdminProvider) => {
    setBusyId(p.id);
    try {
      const writes = providersRef.current.map((x) => {
        const w = toWrite(x);
        if (x.id === p.id) w.api_key = '';
        return w;
      });
      await commit(writes);
      toast('ok', t('providers.keyCleared' as MessageKey));
    } catch {
      toast('danger', t('providers.keyClearFailed' as MessageKey));
    } finally {
      setBusyId(null);
    }
  };

  const onUpdate = async (p: AdminProvider, fields: {
    name: string | null;
    account: string | null;
    kind: string;
    base: string;
    models: string[];
  }) => {
    setBusyId(p.id);
    try {
      const writes = providersRef.current.map((x) => ({
        ...toWrite(x),
        ...(x.id === p.id ? fields : {}),
      }));
      await commit(writes);
      toast('ok', t('providers.configSaved' as MessageKey));
    } catch (e) {
      toast('danger', t('providers.configSaveFailed' as MessageKey));
      throw e;
    } finally {
      setBusyId(null);
    }
  };

  /* 删除 — 乐观移除，失败回滚 */
  const onDelete = async (p: AdminProvider) => {
    const prev = providersRef.current;
    const optimistic = prev.filter((x) => x.id !== p.id);
    setBusyId(p.id);
    setProviders(optimistic);
    try {
      await commit(optimistic.map(toWrite));
      toast('ok', t('prov.deleted', { id: p.id }));
    } catch {
      setProviders(prev);
      toast('danger', t('prov.deleteFailed'));
    } finally {
      setBusyId(null);
    }
  };

  /* 添加（表单 / toml 解析共用） */
  const onAdd = async (provider: AdminProviderWrite) => {
    const prev = providersRef.current;
    if (prev.some((x) => x.id === provider.id)) {
      throw new Error(t('prov.idExists', { id: provider.id }));
    }
    await commit([...prev.map(toWrite), provider]);
    toast('ok', t('prov.added', { id: provider.id }));
    setShowAdd(false);
  };

  const openAdd = (tab: 'form' | 'toml') => {
    setAddTab(tab);
    setShowAdd(true);
  };

  const enabledCount = providers.filter((p) => p.enabled).length;
  const mode = getAdminMode();
  const visibleProviders = useMemo(() => {
    const query = filter.trim().toLocaleLowerCase();
    if (!query) return providers;
    return providers.filter((provider) => [provider.id, provider.name, provider.account, provider.kind, provider.base, ...(provider.models ?? [])]
      .some((value) => value?.toLocaleLowerCase().includes(query)));
  }, [filter, providers]);
  const availability = useMemo(() => summarizeProviderAvailability(providers.map((provider) => provider.id), availabilityEvents), [availabilityEvents, providers]);
  const statusItems = useMemo(() => [{
    id: 'provider-summary',
    label: t('prov.enabledCount', { n: enabledCount, total: providers.length }),
    tone: enabledCount === providers.length ? 'good' as const : 'default' as const,
  }], [enabledCount, providers.length, t]);
  useStatusBarItems(statusItems);

  const card: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
  };
  const btn: React.CSSProperties = {
    fontSize: 'var(--text-sm)',
    background: 'var(--surface-hover)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
    color: 'var(--text)',
    padding: '0.5rem 0.875rem',
  };
  const btnPrimary: React.CSSProperties = {
    ...btn,
    background: 'var(--accent)',
    borderColor: 'var(--accent)',
    color: '#fff',
    fontWeight: 600,
  };

  return (
    <div className="page-container">
      <div className="ui-page-title providers-toolbar">
        <div className="min-w-0">
          <h1>{t('prov.title')}</h1>
          <p className="tabular" title={mode === 'mock' ? t('prov.mockMode') : t('prov.liveMode')}>
            {t('prov.enabledCount', { n: enabledCount, total: providers.length })}
          </p>
        </div>
        <div className="providers-toolbar__tools">
          <button type="button" onClick={() => openAdd('form')} style={btnPrimary}>
            {t('prov.add')}
          </button>
          <button
            type="button"
            onClick={() => openAdd('toml')}
            style={btn}
            title={t('add.pasteHint')}
          >
            {t('prov.pasteToml')}
          </button>
          {!loading && !error && providers.length > 0 && <label className="providers-toolbar__search ui-surface flex min-h-10 items-center gap-2 px-3">
            <Search size={16} aria-hidden="true" style={{ color: 'var(--text-muted)' }} />
            <span className="sr-only">{t('providers.search' as MessageKey)}</span>
            <input autoFocus type="search" value={filter} onChange={(event) => setFilter(event.target.value)} placeholder={t('providers.search' as MessageKey)} aria-label={t('providers.search' as MessageKey)} className="min-w-0 flex-1 bg-transparent text-sm outline-none" />
            <span className="tabular text-xs" style={{ color: 'var(--text-subtle)' }}>{visibleProviders.length}/{providers.length}</span>
          </label>}
        </div>
      </div>

      <div className="space-y-4">
        {showAdd && (
          <AddProviderPanel
            initialTab={addTab}
            onAdd={onAdd}
            onCancel={() => setShowAdd(false)}
          />
        )}

        {loading ? (
          <div className="provider-grid" aria-busy="true">
            <div className="h-48 animate-pulse" style={card} />
            <div className="h-48 animate-pulse" style={card} />
          </div>
        ) : error ? (
          <section
            className="fade-in p-6"
            style={{ ...card, borderColor: 'var(--danger)', background: 'var(--danger-bg)' }}
            role="alert"
          >
            <p style={{ fontSize: 'var(--text-sm)', color: 'var(--danger)' }}>
              {t('common.loadFailed')}
              {error}
            </p>
            <button type="button" onClick={load} style={{ ...btn, marginTop: '0.75rem' }}>
              {t('common.retry')}
            </button>
          </section>
        ) : providers.length === 0 ? (
          <section className="fade-in p-10 text-center" style={card}>
            <p className="font-semibold" style={{ fontSize: 'var(--text-lg)' }}>
              {t('prov.empty')}
            </p>
            <p
              className="mx-auto mt-2 max-w-md"
              style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}
            >
              {t('prov.emptyHint')}
            </p>
            <div className="mt-5 flex justify-center gap-3">
              <button type="button" onClick={() => openAdd('form')} style={btnPrimary}>
                {t('prov.add')}
              </button>
              <button type="button" onClick={() => openAdd('toml')} style={btn}>
                {t('prov.pasteToml')}
              </button>
            </div>
          </section>
        ) : (
          visibleProviders.length === 0 ? <p className="py-8 text-center text-sm" style={{ color: 'var(--text-muted)' }}>{t('providers.noSearchMatches' as MessageKey)}</p> : <div className="provider-grid">
            {visibleProviders.map((p) => (
              <ProviderCard
                key={p.id}
                provider={p}
                availability={availability[p.id]}
                busy={busyId === p.id}
                onToggle={(prov, en) => void onToggle(prov, en)}
                onReplaceKey={onReplaceKey}
                onClearKey={(prov) => void onClearKey(prov)}
                onUpdate={onUpdate}
                onDelete={(prov) => void onDelete(prov)}
              />
            ))}
          </div>
        )}
      </div>
      <ConfigFileSync disabled={busyId !== null || showAdd} onImported={load} />
    </div>
  );
}
