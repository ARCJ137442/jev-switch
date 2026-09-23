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
 * Playground 页（v2 设计系统 · docs/design/UI-AUDIT-2026-09-23.md）：
 * 控制台内页定位 —— 页标题 --text-2xl（与其他三页一致），长说明收进标题旁 `?` tooltip，
 * 删掉营销 hero / MVP badge / 常驻 Endpoint 技术面板，curl 示例默认折叠（渐进披露）。
 * 双栏 Input/Output + 示例 chips + Run Jev（Cmd/Ctrl+Enter）。
 */
export function PlaygroundPage() {
  const { t } = useI18n();
  const [model, setModel] = useState<string>(DEFAULT_MODEL_OPTIONS[0].value);
  const [serverModels, setServerModels] = useState<ModelEntry[]>([]);
  const [showCurl, setShowCurl] = useState(false);
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

  const card: React.CSSProperties = {
    background: 'var(--surface)',
    border: '1px solid var(--border)',
    borderRadius: 'var(--radius)',
  };

  return (
    <div className="mx-auto max-w-7xl px-6 py-8">
      {/* 页标题 —— 长说明不常驻占屏，收进 `?` tooltip（审查报告 §Hero 收敛） */}
      <div className="mb-6 flex items-center gap-2">
        <h1 className="font-semibold" style={{ fontSize: 'var(--text-2xl)' }}>
          {t('shell.navPlayground')}
        </h1>
        <span
          role="note"
          tabIndex={0}
          aria-label={t('pg.heroLead')}
          title={t('pg.heroLead')}
          className="inline-flex h-5 w-5 cursor-help items-center justify-center"
          style={{
            borderRadius: '50%',
            border: '1px solid var(--border)',
            background: 'var(--surface)',
            color: 'var(--text-muted)',
            fontSize: 'var(--text-xs)',
          }}
        >
          ?
        </span>
      </div>

      {/* Main playground — 双栏 */}
      <div className="grid grid-cols-1 gap-6 lg:grid-cols-3">
        {/* Left column — model */}
        <aside className="flex flex-col gap-4 lg:col-span-1">
          <ModelSelect
            options={modelOptions}
            value={model}
            onChange={setModel}
            serverReachable={server.status === 'ok'}
          />
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

          {/* Quick start — curl 默认折叠（渐进披露，不一上来糊一屏代码） */}
          <section className="fade-in overflow-hidden px-4 py-3" style={card}>
            <div className="flex items-baseline justify-between gap-3">
              <h2 className="font-semibold" style={{ fontSize: 'var(--text-sm)' }}>
                {t('pg.quickStart')}
              </h2>
              <button
                type="button"
                onClick={() => setShowCurl((v) => !v)}
                aria-expanded={showCurl}
                style={{
                  fontSize: 'var(--text-sm)',
                  background: 'transparent',
                  border: 'none',
                  color: 'var(--accent)',
                  cursor: 'pointer',
                  padding: 0,
                }}
              >
                {showCurl ? t('pg.hideCurl') : t('pg.showCurl')}
              </button>
            </div>
            {showCurl && (
              <pre
                className="fade-in mt-3 overflow-x-auto p-3 leading-relaxed"
                style={{
                  border: '1px solid var(--border)',
                  borderRadius: 'var(--radius)',
                  background: 'var(--surface-hover)',
                  fontFamily: 'var(--font-mono)',
                  fontSize: 'var(--text-xs)',
                  color: 'var(--text)',
                }}
              >
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
            )}
          </section>
        </div>
      </div>
    </div>
  );
}
