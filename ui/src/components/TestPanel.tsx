import { useState } from 'react';
import { postSystemOne, type SystemOneRequest, type SystemOneResponse } from '../api';

interface Props {
  model: string;
  defaultState: string;
  defaultQuestions: string;
}

type Status = 'idle' | 'loading' | 'ok' | 'error';

export function TestPanel({ model, defaultState, defaultQuestions }: Props) {
  const [stateJson, setStateJson] = useState<string>(defaultState);
  const [questionsJson, setQuestionsJson] = useState<string>(defaultQuestions);
  const [status, setStatus] = useState<Status>('idle');
  const [response, setResponse] = useState<SystemOneResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [latencyMs, setLatencyMs] = useState<number | null>(null);

  const onTest = async () => {
    setStatus('loading');
    setError(null);
    setResponse(null);
    setLatencyMs(null);

    let parsedState: unknown;
    let parsedQuestions: unknown;
    try {
      parsedState = JSON.parse(stateJson || '{}');
    } catch (e) {
      setStatus('error');
      setError(`state JSON parse failed: ${(e as Error).message}`);
      return;
    }
    try {
      parsedQuestions = JSON.parse(questionsJson || '{}');
    } catch (e) {
      setStatus('error');
      setError(`questions JSON parse failed: ${(e as Error).message}`);
      return;
    }

    const req: SystemOneRequest = {
      model,
      state: parsedState as Record<string, unknown>,
      questions: parsedQuestions as SystemOneRequest['questions'],
    };

    const t0 = performance.now();
    try {
      const res = await postSystemOne(req);
      setResponse(res);
      setLatencyMs(Math.round(performance.now() - t0));
      setStatus('ok');
    } catch (e) {
      setStatus('error');
      setError((e as Error).message);
      setLatencyMs(Math.round(performance.now() - t0));
    }
  };

  return (
    <section className="rounded-lg border border-border bg-panel p-4">
      <header className="mb-3 flex items-center justify-between">
        <h2 className="text-sm font-semibold uppercase tracking-wider text-neutral-400">Test</h2>
        <button
          type="button"
          onClick={onTest}
          disabled={status === 'loading'}
          className="rounded bg-accent px-3 py-1.5 text-sm font-medium text-black transition-opacity hover:opacity-90 disabled:cursor-not-allowed disabled:opacity-50"
        >
          {status === 'loading' ? 'Testing…' : 'Test'}
        </button>
      </header>

      <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
        <div>
          <label className="mb-1 block text-xs text-neutral-400">state (JSON)</label>
          <textarea
            value={stateJson}
            onChange={(e) => setStateJson(e.target.value)}
            spellCheck={false}
            className="h-32 w-full resize-y rounded border border-border bg-black/40 p-2 font-mono text-xs text-neutral-100 outline-none focus:border-accent"
          />
        </div>
        <div>
          <label className="mb-1 block text-xs text-neutral-400">questions (JSON)</label>
          <textarea
            value={questionsJson}
            onChange={(e) => setQuestionsJson(e.target.value)}
            spellCheck={false}
            className="h-32 w-full resize-y rounded border border-border bg-black/40 p-2 font-mono text-xs text-neutral-100 outline-none focus:border-accent"
          />
        </div>
      </div>

      <div className="mt-3">
        <div className="mb-1 flex items-center gap-2">
          <span className="text-xs text-neutral-400">Response</span>
          <span
            className={
              'rounded px-1.5 py-0.5 font-mono text-[10px] uppercase ' +
              (status === 'idle'
                ? 'bg-neutral-800 text-neutral-500'
                : status === 'loading'
                  ? 'bg-neutral-800 text-neutral-300'
                  : status === 'ok'
                    ? 'bg-accent/20 text-accent'
                    : 'bg-red-900/40 text-red-300')
            }
          >
            {status}
          </span>
          {latencyMs !== null && (
            <span className="font-mono text-[10px] text-neutral-500">{latencyMs} ms</span>
          )}
        </div>
        <pre className="max-h-72 overflow-auto rounded border border-border bg-black/50 p-3 font-mono text-xs text-neutral-200">
          {error
            ? `// error\n${error}`
            : response
              ? JSON.stringify(response, null, 2)
              : '// click "Test" to send POST /v1/systemone'}
        </pre>
      </div>
    </section>
  );
}