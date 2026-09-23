import { useEffect, useRef, useState } from 'react';
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
import { useToast } from '../app/feedback';
import { ProviderCard } from '../components/providers/ProviderCard';
import { AddProviderPanel } from '../components/providers/AddProviderPanel';
import { useI18n } from '../i18n';

const toWrite = (p: AdminProvider): AdminProviderWrite => ({
  id: p.id,
  kind: p.kind,
  base: p.base,
  enabled: p.enabled,
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
    <div className="mx-auto max-w-7xl px-6 py-8">
      <div className="mb-6 flex flex-wrap items-baseline justify-between gap-2">
        <h1 className="font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>
          {t('prov.title')}
        </h1>
        <span
          className="tabular"
          style={{ fontSize: 'var(--text-sm)', color: 'var(--text-muted)' }}
          title={mode === 'mock' ? t('prov.mockMode') : t('prov.liveMode')}
        >
          {t('prov.enabledCount', { n: enabledCount, total: providers.length })}
        </span>
      </div>

      <div className="mb-6 flex flex-wrap gap-3">
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
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2" aria-busy="true">
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
          <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
            {providers.map((p) => (
              <ProviderCard
                key={p.id}
                provider={p}
                busy={busyId === p.id}
                onToggle={(prov, en) => void onToggle(prov, en)}
                onReplaceKey={onReplaceKey}
                onDelete={(prov) => void onDelete(prov)}
              />
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
