import { useEffect, useMemo, useState } from 'react';
import { ModelSelect } from '../components/playground/ModelSelect';
import { TestPanel } from '../components/playground/TestPanel';
import { ExampleChips } from '../components/playground/ExampleChips';
import { useServerStatus } from '../app/Shell';
import {
  BASE,
  DEFAULT_MODEL_OPTIONS,
  fetchModels,
  type ModelEntry,
} from '../api';
import { EXAMPLES, type ExamplePayload } from '../examples';
import { useI18n } from '../i18n';

/**
 * Playground 页 — 保留原单页 DNA（jevplayground 黑白等宽），
 * 双栏 Input/Output + 示例 chips + Run Jev（design/01 §6.3 × TeamSense 面板圆角）。
 * 导航 / daemon 灯 / footer 已上移至 Shell。
 */
export function PlaygroundPage() {
  const { t } = useI18n();
  const [model, setModel] = useState<string>(DEFAULT_MODEL_OPTIONS[0].value);
  const [serverModels, setServerModels] = useState<ModelEntry[]>([]);
  const server = useServerStatus();

  // Inputs — lifted up so example chips can rewrite them
  const [stateJson, setStateJson] = useState<string>(
    JSON.stringify(EXAMPLES[0].payload.state, null, 2),
  );
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
          // Runtime defense: shape is frozen (contracts/05 §2), but never
          // crash the page — fall back to the bundled defaults.
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
    // 当 server 返回时, 用 server 列表（upstream 取契约字段）, 去重
    const seen = new Set<string>();
    return serverModels
      .map((m) => ({
        value: m.id,
        label: m.id,
        upstream: m.upstream,
      }))
      .filter((o) => {
        if (seen.has(o.value)) return false;
        seen.add(o.value);
        return true;
      });
  }, [serverModels]);

  const onPickExample = (ex: ExamplePayload) => {
    setActiveExampleId(ex.id);
    setStateJson(JSON.stringify(ex.payload.state, null, 2));
    setQuestionsJson(JSON.stringify(ex.payload.questions, null, 2));
    // UI-1：样例与模型选择解耦 — 切样例**不**改用户的 model 选择；
    // payload.model 仅作初始默认值记录（初始 model 见 useState DEFAULT）。
  };

  return (
    <div className="bg-bg text-ink">
      {/* Hero — 大标题, 等宽字体, 主蓝点缀（B4） */}
      <section className="border-b border-border">
        <div className="mx-auto max-w-7xl px-6 py-12 md:py-16">
          <div className="max-w-3xl">
            <div className="mb-3 inline-flex items-center gap-2 rounded border border-border bg-panel px-2 py-1 font-mono text-[10px] uppercase tracking-widest text-inkMuted">
              <span className="inline-block h-1.5 w-1.5 rounded-full bg-primaryFill" />
              MVP · M0.10
            </div>
            <h1 className="font-mono text-4xl font-semibold leading-[1.05] tracking-tightest text-ink md:text-5xl">
              One protocol.{' '}
              <span className="text-primary">Every upstream.</span>
            </h1>
            <p className="mt-4 max-w-2xl text-sm leading-relaxed text-inkMuted">
              {t('pg.heroLead')}
            </p>
          </div>
        </div>
      </section>

      {/* Main playground — 双栏 */}
      <main className="mx-auto max-w-7xl px-6 py-8">
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
          {/* Left column — model + endpoint */}
          <aside className="flex flex-col gap-4 lg:col-span-1">
            <ModelSelect
              options={modelOptions}
              value={model}
              onChange={setModel}
              serverReachable={server.status === 'ok'}
            />

            {/* Endpoint box — TeamSense 面板三段 */}
            <section className="overflow-hidden rounded-card border border-border bg-panel">
              <header className="border-b border-border px-4 py-2.5">
                <h2 className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted">
                  Endpoint
                </h2>
              </header>
              <div className="space-y-2 px-4 py-3 font-mono text-xs">
                <Row label="POST" value="/v1/systemone" />
                <Row label="GET" value="/v1/models" />
                <Row label="GET" value="/health" />
                <div className="mt-2 border-t border-border pt-2 text-inkMuted">
                  base = <span className="text-ink">{BASE}</span>
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
            <section className="overflow-hidden rounded-card border border-border bg-panel px-4 py-3">
              <div className="mb-2 flex items-baseline justify-between">
                <h2 className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkMuted">
                  {t('pg.quickStart')}
                </h2>
                <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
                  curl
                </span>
              </div>
              <pre className="overflow-x-auto border border-border bg-soft p-3 font-mono text-xs leading-relaxed text-ink">
{`curl -X POST ${BASE}/v1/systemone \\
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
    </div>
  );
}

function Row({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-baseline gap-2">
      <span className="w-10 text-inkMuted">{label}</span>
      <span className="text-ink">{value}</span>
    </div>
  );
}
