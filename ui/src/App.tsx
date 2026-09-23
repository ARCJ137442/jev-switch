import { useEffect, useMemo, useState } from 'react';
import { Header, useServerStatus } from './components/Header';
import { ProvidersPanel } from './components/ProvidersPanel';
import { ModelSelect } from './components/ModelSelect';
import { TestPanel } from './components/TestPanel';
import { ExampleChips } from './components/ExampleChips';
import {
  DEFAULT_MODEL_OPTIONS,
  PROVIDERS,
  fetchModels,
  type ModelInfo,
  type ProviderInfo,
} from './api';
import { EXAMPLES, type ExamplePayload } from './examples';

/**
 * 主布局 — 参考 jevplayground.com Linear/Stripe 文档风
 * - 顶部: Logo + 站名 + 水平导航 + 状态条
 * - Hero: 大标题 + 副标题
 * - 双栏 (Input | Output)
 * - 4 个示例 chips
 * - 大黑色 CTA "Run Jev ↗"
 * - 底部 footer: 版本 + endpoint
 */
export default function App() {
  const [providers, setProviders] = useState<ProviderInfo[]>(PROVIDERS);
  const [model, setModel] = useState<string>(DEFAULT_MODEL_OPTIONS[0].value);
  const [serverModels, setServerModels] = useState<ModelInfo[]>([]);
  const server = useServerStatus();

  // Inputs — lifted up so example chips can rewrite them
  const [stateJson, setStateJson] = useState<string>(JSON.stringify(EXAMPLES[0].payload.state, null, 2));
  const [questionsJson, setQuestionsJson] = useState<string>(
    JSON.stringify(EXAMPLES[0].payload.questions, null, 2),
  );
  const [activeExampleId, setActiveExampleId] = useState<string>(EXAMPLES[0].id);

  useEffect(() => {
    if (server.status !== 'ok') return;
    let cancelled = false;
    fetchModels()
      .then((m) => {
        if (!cancelled) {
          // Defensive: `data` may be missing if the upstream returns an
          // unexpected shape; always fall back to the bundled defaults.
          const next = Array.isArray(m?.data) ? m.data : [];
          setServerModels(next);
        }
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [server.status]);

  const modelOptions = useMemo(() => {
    if (!Array.isArray(serverModels) || serverModels.length === 0) {
      return DEFAULT_MODEL_OPTIONS.map((o) => ({
        value: o.value,
        label: o.value,
        upstream: o.value.startsWith('laya') ? 'laya' : 'vercel',
      }));
    }
    // 当 server 返回时, 用 server 列表, 去重
    const seen = new Set<string>();
    return serverModels
      .map((m) => ({
        value: m.id,
        label: m.id,
        upstream: m.id.startsWith('laya') ? 'laya' : 'vercel',
      }))
      .filter((o) => {
        if (seen.has(o.value)) return false;
        seen.add(o.value);
        return true;
      });
  }, [serverModels]);

  const onToggleProvider = (id: ProviderInfo['id'], enabled: boolean) => {
    setProviders((prev) => prev.map((p) => (p.id === id ? { ...p, enabled } : p)));
  };

  const onPickExample = (ex: ExamplePayload) => {
    setActiveExampleId(ex.id);
    setStateJson(JSON.stringify(ex.payload.state, null, 2));
    setQuestionsJson(JSON.stringify(ex.payload.questions, null, 2));
    setModel(ex.payload.model);
  };

  return (
    <div className="min-h-full bg-bg text-ink">
      <Header
        serverStatus={server.status}
        modelsCount={modelOptions.length}
        providersEnabled={providers.filter((p) => p.enabled).length}
        providersTotal={providers.length}
      />

      {/* Hero — 大标题, 等宽字体, 强黑白对比 */}
      <section className="border-b border-border">
        <div className="mx-auto max-w-7xl px-6 py-12 md:py-16">
          <div className="max-w-3xl">
            <div className="mb-3 inline-flex items-center gap-2 border border-border bg-panel px-2 py-1 font-mono text-[10px] uppercase tracking-widest text-inkMuted">
              <span className="inline-block h-1.5 w-1.5 bg-ink" />
              MVP · M0.10
            </div>
            <h1 className="font-mono text-4xl font-semibold leading-[1.05] tracking-tightest text-ink md:text-5xl">
              One protocol.{' '}
              <span className="text-inkMuted">Every upstream.</span>
            </h1>
            <p className="mt-4 max-w-2xl text-base leading-relaxed text-inkMuted">
              Route a single Jev SystemOne call across heterogeneous upstream models — Vercel AI
              Gateway, local Laya daemon, or any future provider. Switch the{' '}
              <code className="border border-border bg-panel px-1.5 py-0.5 font-mono text-xs text-ink">
                model
              </code>{' '}
              field; the router handles protocol translation.
            </p>
          </div>
        </div>
      </section>

      {/* Main playground — 双栏 */}
      <main className="mx-auto max-w-7xl px-6 py-8">
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
          {/* Left column — providers + model */}
          <aside className="flex flex-col gap-4 lg:col-span-1">
            <ModelSelect
              options={modelOptions}
              value={model}
              onChange={setModel}
              serverReachable={server.status === 'ok'}
            />
            <ProvidersPanel providers={providers} onToggle={onToggleProvider} />

            {/* Endpoint box */}
            <section className="border border-border bg-panel">
              <header className="border-b border-border px-4 py-2.5">
                <h2 className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
                  Endpoint
                </h2>
              </header>
              <div className="space-y-2 p-4 font-mono text-xs">
                <Row label="POST" value="/v1/systemone" />
                <Row label="GET" value="/v1/models" />
                <Row label="GET" value="/health" />
                <div className="mt-2 border-t border-border pt-2 text-inkSubtle">
                  base ={' '}
                  <span className="text-ink">http://127.0.0.1:8765</span>
                </div>
              </div>
            </section>
          </aside>

          {/* Right column — TestPanel + example chips */}
          <div className="flex flex-col gap-4 lg:col-span-2">
            <ExampleChips examples={EXAMPLES} onPick={onPickExample} activeId={activeExampleId} />

            <TestPanel
              model={model}
              stateJson={stateJson}
              questionsJson={questionsJson}
              onStateChange={setStateJson}
              onQuestionsChange={setQuestionsJson}
            />

            {/* Quick start hint */}
            <section className="border border-border bg-panel p-4">
              <div className="mb-2 flex items-baseline justify-between">
                <h2 className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
                  Quick start
                </h2>
                <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
                  curl
                </span>
              </div>
              <pre className="overflow-x-auto border border-border bg-bg p-3 font-mono text-[11px] leading-relaxed text-ink">
{`curl -X POST http://127.0.0.1:8765/v1/systemone \\
  -H "Content-Type: application/json" \\
  -d '{
    "model": "${model}",
    "state": { "test": true },
    "questions": {
      "q": {
        "type": "noul",
        "instructions": "is this a test?",
        "criteria": { "true": "yes", "false": "no" }
      }
    }
  }'`}
              </pre>
            </section>
          </div>
        </div>
      </main>

      <footer className="mt-12 border-t border-border bg-bg">
        <div className="mx-auto flex max-w-7xl flex-col items-start justify-between gap-2 px-6 py-5 md:flex-row md:items-center">
          <div className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
            jev-switch · mvp · m0.10
          </div>
          <div className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
            built with axum · react · tailwind
          </div>
        </div>
      </footer>
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline gap-2">
      <span className="w-10 text-inkSubtle">{label}</span>
      <span className="text-ink">{value}</span>
    </div>
  );
}
