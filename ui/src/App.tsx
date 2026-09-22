import { useEffect, useMemo, useState } from 'react';
import { ProvidersPanel } from './components/ProvidersPanel';
import { ModelSelect } from './components/ModelSelect';
import { TestPanel } from './components/TestPanel';
import {
  DEFAULT_MODEL_OPTIONS,
  PROVIDERS,
  SAMPLE_QUESTIONS,
  SAMPLE_STATE,
  fetchHealth,
  fetchModels,
  type ModelInfo,
  type ProviderInfo,
} from './api';

export default function App() {
  const [providers, setProviders] = useState<ProviderInfo[]>(PROVIDERS);
  const [model, setModel] = useState<string>(DEFAULT_MODEL_OPTIONS[0].value);
  const [serverModels, setServerModels] = useState<ModelInfo[]>([]);
  const [health, setHealth] = useState<'unknown' | 'ok' | 'error'>('unknown');
  const [healthText, setHealthText] = useState<string>('');

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const text = await fetchHealth();
        if (!cancelled) {
          setHealth('ok');
          setHealthText(text);
        }
      } catch (e) {
        if (!cancelled) {
          setHealth('error');
          setHealthText((e as Error).message);
        }
      }
      try {
        const m = await fetchModels();
        if (!cancelled) setServerModels(m.data);
      } catch {
        /* models endpoint may not be ready yet — silently ignore */
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const modelOptions = useMemo(() => {
    if (serverModels.length === 0) return DEFAULT_MODEL_OPTIONS;
    const fromServer = serverModels.map((m) => ({
      value: m.id,
      label: `${m.id}  (server)`,
    }));
    // de-dup by value
    const seen = new Set<string>();
    return [...fromServer, ...DEFAULT_MODEL_OPTIONS].filter((o) => {
      if (seen.has(o.value)) return false;
      seen.add(o.value);
      return true;
    });
  }, [serverModels]);

  const onToggleProvider = (id: ProviderInfo['id'], enabled: boolean) => {
    setProviders((prev) => prev.map((p) => (p.id === id ? { ...p, enabled } : p)));
  };

  return (
    <div className="mx-auto flex min-h-full max-w-3xl flex-col gap-4 p-6">
      <header className="flex items-baseline justify-between">
        <h1 className="text-xl font-semibold tracking-tight text-neutral-100">Jev-Switch MVP</h1>
        <div className="flex items-center gap-2 text-xs">
          <span
            className={
              'inline-block h-2 w-2 rounded-full ' +
              (health === 'ok' ? 'bg-accent' : health === 'error' ? 'bg-red-500' : 'bg-neutral-600')
            }
          />
          <span className="font-mono text-neutral-400">
            {health === 'ok' ? healthText || '/health OK' : health === 'error' ? 'daemon unreachable' : 'checking…'}
          </span>
        </div>
      </header>

      <ProvidersPanel providers={providers} onToggle={onToggleProvider} />
      <ModelSelect options={modelOptions} value={model} onChange={setModel} />
      <TestPanel model={model} defaultState={SAMPLE_STATE} defaultQuestions={SAMPLE_QUESTIONS} />

      <footer className="pt-2 text-center text-[11px] text-neutral-600">
        POST http://127.0.0.1:8765/v1/systemone · M0.10
      </footer>
    </div>
  );
}