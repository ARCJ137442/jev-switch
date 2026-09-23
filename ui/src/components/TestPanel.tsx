import { useState } from 'react';
import { postSystemOne, type SystemOneRequest, type SystemOneResponse } from '../api';

interface Props {
  model: string;
  stateJson: string;
  questionsJson: string;
  onStateChange: (v: string) => void;
  onQuestionsChange: (v: string) => void;
  onRunningChange?: (running: boolean) => void;
}

type Status = 'idle' | 'loading' | 'ok' | 'error';

/**
 * 双栏布局的核心面板 — 左侧 Input，右侧 Output
 * 极简风 — 顶部 tab 切换 (Single / Multiple question 风格)，底部大黑色 CTA "Run Jev"
 */
export function TestPanel({
  model,
  stateJson,
  questionsJson,
  onStateChange,
  onQuestionsChange,
  onRunningChange,
}: Props) {
  const [status, setStatus] = useState<Status>('idle');
  const [response, setResponse] = useState<SystemOneResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [latencyMs, setLatencyMs] = useState<number | null>(null);
  const [inputMode, setInputMode] = useState<'single' | 'multiple'>('single');

  // Token 计数 — 极简近似: characters / 4
  const stateTokens = Math.ceil((stateJson.length || 0) / 4);
  const questionsTokens = Math.ceil((questionsJson.length || 0) / 4);
  const totalInputTokens = stateTokens + questionsTokens;

  const onRun = async () => {
    setStatus('loading');
    setError(null);
    setResponse(null);
    setLatencyMs(null);
    onRunningChange?.(true);

    let parsedState: unknown;
    let parsedQuestions: unknown;
    try {
      parsedState = JSON.parse(stateJson || '{}');
    } catch (e) {
      setStatus('error');
      setError(`state JSON parse failed: ${(e as Error).message}`);
      onRunningChange?.(false);
      return;
    }
    try {
      parsedQuestions = JSON.parse(questionsJson || '{}');
    } catch (e) {
      setStatus('error');
      setError(`questions JSON parse failed: ${(e as Error).message}`);
      onRunningChange?.(false);
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
    } finally {
      onRunningChange?.(false);
    }
  };

  return (
    <section id="playground" className="border border-border bg-panel">
      {/* Top bar — input mode tabs */}
      <header className="flex items-center justify-between border-b border-border px-4 py-2">
        <div className="flex items-center gap-1">
          <TabButton active={inputMode === 'single'} onClick={() => setInputMode('single')}>
            Single question
          </TabButton>
          <TabButton active={inputMode === 'multiple'} onClick={() => setInputMode('multiple')}>
            Multiple questions
          </TabButton>
        </div>
        <div className="flex items-center gap-3 font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
          <span className="tabular">~{totalInputTokens} tokens</span>
          <span className="text-inkSubtle">·</span>
          <span>{Object.keys(JSON.parse(questionsJson || '{}') || {}).length || 0} question(s)</span>
        </div>
      </header>

      <div className="grid grid-cols-1 divide-border lg:grid-cols-2 lg:divide-x">
        {/* INPUT COLUMN */}
        <div className="flex flex-col">
          <div className="flex items-center justify-between border-b border-border px-4 py-2">
            <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
              Input
            </span>
            <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
              JSON
            </span>
          </div>

          <div className="flex flex-col gap-4 p-4">
            <Field
              label="state"
              hint={`${stateTokens} tokens`}
              hintBg
              raw={
                <textarea
                  value={stateJson}
                  onChange={(e) => onStateChange(e.target.value)}
                  spellCheck={false}
                  rows={6}
                  className="w-full resize-y border border-border bg-bg p-2.5 font-mono text-xs leading-relaxed text-ink"
                />
              }
            />

            <Field
              label={inputMode === 'single' ? 'question' : 'questions'}
              hint={`${questionsTokens} tokens`}
              raw={
                <textarea
                  value={questionsJson}
                  onChange={(e) => onQuestionsChange(e.target.value)}
                  spellCheck={false}
                  rows={inputMode === 'multiple' ? 12 : 8}
                  className="w-full resize-y border border-border bg-bg p-2.5 font-mono text-xs leading-relaxed text-ink"
                />
              }
            />

            {/* Decision type hint */}
            <DecisionTypeHint questionsJson={questionsJson} />
          </div>
        </div>

        {/* OUTPUT COLUMN */}
        <div className="flex flex-col">
          <div className="flex items-center justify-between border-b border-border px-4 py-2">
            <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
              Output
            </span>
            <span className="flex items-center gap-2 font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
              <span
                className={
                  'inline-block h-1.5 w-1.5 ' +
                  (status === 'idle'
                    ? 'bg-inkSubtle'
                    : status === 'loading'
                      ? 'animate-pulse bg-ink'
                      : status === 'ok'
                        ? 'bg-emerald-500'
                        : 'bg-red-500')
                }
              />
              {status}
              {latencyMs !== null && <span className="tabular text-inkSubtle">· {latencyMs}ms</span>}
            </span>
          </div>

          <div className="flex-1 p-4">
            <pre className="min-h-[200px] whitespace-pre-wrap break-words border border-border bg-bg p-3 font-mono text-xs leading-relaxed text-ink">
              {error
                ? `// error\n${error}`
                : response
                  ? JSON.stringify(response, null, 2)
                  : '// [ · ]\n// waiting for input + Run Jev'}
            </pre>
          </div>

          {/* Answer summary — only when ok */}
          {status === 'ok' && response && <AnswerSummary response={response} />}
        </div>
      </div>

      {/* Big black CTA */}
      <div className="flex items-center justify-between border-t border-border bg-bg px-4 py-3">
        <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
          POST http://127.0.0.1:8765/v1/systemone
        </span>
        <button
          type="button"
          onClick={onRun}
          disabled={status === 'loading'}
          className="inline-flex items-center gap-2 border border-ink bg-ink px-5 py-2 font-mono text-sm font-semibold text-bg transition-colors hover:bg-bg hover:text-ink disabled:cursor-not-allowed disabled:opacity-50"
        >
          {status === 'loading' ? 'Running…' : 'Run Jev'}
          {status !== 'loading' && <span aria-hidden>↗</span>}
        </button>
      </div>
    </section>
  );
}

/* ---------- 内部小组件 ---------- */

function TabButton({
  active,
  onClick,
  children,
}: {
  active: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className={
        'px-2.5 py-1 font-mono text-xs transition-colors ' +
        (active
          ? 'border border-ink bg-ink text-bg'
          : 'border border-transparent text-inkMuted hover:text-ink')
      }
    >
      {children}
    </button>
  );
}

function Field({
  label,
  hint,
  hintBg,
  raw,
}: {
  label: string;
  hint?: string;
  hintBg?: boolean;
  raw: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-baseline justify-between">
        <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
          {label}
        </span>
        {hint && (
          <span
            className={
              'font-mono text-[10px] uppercase tracking-widest ' +
              (hintBg ? 'border border-border bg-bg px-1.5 py-0.5 text-inkMuted' : 'text-inkSubtle')
            }
          >
            {hint}
          </span>
        )}
      </div>
      {raw}
    </div>
  );
}

function DecisionTypeHint({ questionsJson }: { questionsJson: string }) {
  let parsed: unknown;
  try {
    parsed = JSON.parse(questionsJson || '{}');
  } catch {
    return (
      <div className="border border-red-300 bg-red-50 px-3 py-2 font-mono text-[10px] uppercase tracking-widest text-red-700">
        invalid JSON
      </div>
    );
  }
  if (!parsed || typeof parsed !== 'object') return null;
  const entries = Object.values(parsed as Record<string, { type?: string }>);
  if (entries.length === 0) return null;
  return (
    <div className="flex flex-wrap items-center gap-2 border border-border bg-bg px-3 py-2">
      <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
        decision type
      </span>
      {entries.map((q, i) => (
        <span
          key={i}
          className="border border-ink bg-ink px-2 py-0.5 font-mono text-[10px] uppercase tracking-widest text-bg"
        >
          {q.type ?? 'unknown'}
        </span>
      ))}
    </div>
  );
}

function AnswerSummary({ response }: { response: SystemOneResponse }) {
  const answers = Object.entries(response.answers ?? {});
  if (answers.length === 0) return null;
  return (
    <div className="border-t border-border px-4 py-3">
      <div className="mb-2 font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
        Summary
      </div>
      <ul className="space-y-1.5 font-mono text-xs">
        {answers.map(([qid, a]) => {
          const value =
            a.choice ??
            a.noul ??
            a.boolean ??
            a.score ??
            (a.probabilities ? Object.entries(a.probabilities).sort((x, y) => y[1] - x[1])[0]?.[0] : null) ??
            '—';
          const conf = a.confidence != null ? a.confidence.toFixed(2) : null;
          return (
            <li key={qid} className="flex items-baseline gap-2">
              <span className="text-inkSubtle">{qid}</span>
              <span className="text-inkSubtle">→</span>
              <span className="font-semibold text-ink tabular">
                {typeof value === 'number' ? value.toFixed(3) : String(value)}
              </span>
              {conf && <span className="text-inkSubtle">conf={conf}</span>}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
