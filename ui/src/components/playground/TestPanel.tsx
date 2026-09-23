import { useState } from 'react';
import {
  BASE,
  postSystemOne,
  SystemOneError,
  type JevRequest,
  type JevResponse,
} from '../../api';

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
  const [response, setResponse] = useState<JevResponse | null>(null);
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

    const req: JevRequest = {
      model,
      // state 语义 = unknown（contracts/01 §2：必填，允许 null/标量 — 编辑器解析后直发）
      state: parsedState,
      questions: parsedQuestions as JevRequest['questions'],
    };

    const t0 = performance.now();
    try {
      const res = await postSystemOne(req);
      setResponse(res);
      setLatencyMs(Math.round(performance.now() - t0));
      setStatus('ok');
    } catch (e) {
      setStatus('error');
      setError(formatSystemOneError(e));
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
                        ? 'bg-ok'
                        : 'bg-danger')
                }
              />
              {status}
              {latencyMs !== null && <span className="tabular text-inkSubtle">· {latencyMs}ms</span>}
            </span>
          </div>

          <div className="flex-1 p-4">
            <pre
              className={
                'min-h-[200px] whitespace-pre-wrap break-words border bg-bg p-3 font-mono text-xs leading-relaxed ' +
                // design/01 §7：错误态输出面板 danger
                (error ? 'border-danger text-danger' : 'border-border text-ink')
              }
            >
              {error
                ? `// error\n${error}`
                : response
                  ? JSON.stringify(response, null, 2)
                  : '// [ · ]\n// waiting for input + Run Jev'}
            </pre>
          </div>

          {/* Answer 分型摘要 + 计量区 — only when ok（design/01 §6.3） */}
          {status === 'ok' && response && (
            <AnswerSummary
              response={response}
              inputTokens={totalInputTokens}
              measuredLatencyMs={latencyMs}
            />
          )}
        </div>
      </div>

      {/* Big black CTA */}
      <div className="flex items-center justify-between border-t border-border bg-bg px-4 py-3">
        <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
          POST {BASE}/v1/systemone
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
      <div className="border border-danger bg-danger/10 px-3 py-2 font-mono text-[10px] uppercase tracking-widest text-danger">
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

/** 数字展示：最多 3 位小数并去尾零（0.960 → 0.96） */
function fmtNum(n: number): string {
  const s = n.toFixed(3);
  return s.includes('.') ? s.replace(/0+$/, '').replace(/\.$/, '') : s;
}

/** cost_usd 展示：0 → `0`；常规值去尾零；极小值科学计数兜底（null → `—` 由调用方处理） */
function fmtCost(n: number): string {
  if (n === 0) return '0';
  const s = n.toFixed(6).replace(/0+$/, '').replace(/\.$/, '');
  return s !== '' && s !== '0' && s !== '-0' ? s : n.toExponential(2);
}

/**
 * design/01 §7 错误文案 — 读 ErrorBody `{error, upstream, retryable}`
 * （contracts/05 §3；与 adminMode/mock 无关，走真实 `/v1/systemone` 错误体）：
 * - 422 → capability 文案
 * - 503 + retryable → `上游限流（retryable）→ 已返回 503`
 * - 其余 → 透传服务端 error / HTTP status
 */
function formatSystemOneError(e: unknown): string {
  if (e instanceof SystemOneError) {
    const up = e.upstream ? ` · upstream=${e.upstream}` : '';
    if (e.status === 422) {
      return `capability 不匹配（422）${up}\n${e.message}`;
    }
    if (e.status === 503 && e.retryable) {
      return `上游限流（retryable）→ 已返回 503${up}\n${e.message}`;
    }
  }
  return e instanceof Error ? e.message : String(e);
}

/** probabilities → 降序 `[key, value]` 列表（非法/空 → null） */
function toProbEntries(raw: unknown): [string, number][] | null {
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) return null;
  const out: [string, number][] = [];
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof v === 'number' && Number.isFinite(v)) out.push([k, v]);
  }
  if (out.length === 0) return null;
  out.sort((a, b) => b[1] - a[1]);
  return out;
}

/**
 * Answer 分型摘要 — 契约分型渲染（B4，contracts/01 §4 / design/01 §6.3）：
 * - choice/score：主值 + `conf` + probabilities 分布条（三字段必填）
 * - noul/boolean：双键来源标注（显示存在者；有效值冻结序 probability > noul），
 *   **无 confidence 行**（§4）
 * - 防御兜底保留：无 `type` → 旧扁平；未知 `type` / 非对象 → 原始 JSON（不崩 UI）
 * + 计量区：usage 优先（snake 显示 / camel 读容忍）、无 usage 本地估算标「估算」、
 *   `upstream_calls`（缺省 1）/ `latency_ms` / `cost_usd`（null → `—`，0 显示 0）
 */
function AnswerSummary({
  response,
  inputTokens,
  measuredLatencyMs,
}: {
  response: JevResponse;
  inputTokens: number;
  measuredLatencyMs: number | null;
}) {
  const answers = response.answers;
  if (!answers || typeof answers !== 'object') {
    return (
      <div className="border-t border-border px-4 py-3">
        <div className="mb-2 font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
          Summary
        </div>
        <pre className="whitespace-pre-wrap break-words border border-border bg-bg p-3 font-mono text-xs text-inkMuted">
          {JSON.stringify(response, null, 2)}
        </pre>
      </div>
    );
  }
  const entries = Object.entries(answers as Record<string, unknown>);
  return (
    <div className="border-t border-border px-4 py-3">
      <div className="mb-2 font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
        Summary
      </div>
      {entries.length > 0 && (
        <ul className="space-y-1.5 font-mono text-xs">
          {entries.map(([qid, raw]) => (
            <AnswerRow key={qid} qid={qid} raw={raw} />
          ))}
        </ul>
      )}
      <Metering
        response={response}
        inputTokens={inputTokens}
        measuredLatencyMs={measuredLatencyMs}
        className={entries.length > 0 ? 'mt-3 border-t border-border pt-2.5' : ''}
      />
    </div>
  );
}

/** 计量区（contracts/01 §5 / contracts/06 §5 / design/01 §6.3 计量行） */
function Metering({
  response,
  inputTokens,
  measuredLatencyMs,
  className,
}: {
  response: JevResponse;
  inputTokens: number;
  measuredLatencyMs: number | null;
  className?: string;
}) {
  const u = response.usage;
  // snake 优先、camel 读容忍（contracts/01 §5 双拼写）
  const pick = (snake: unknown, camel: unknown): number | null =>
    typeof snake === 'number' ? snake : typeof camel === 'number' ? camel : null;
  const inTok = pick(u?.input_tokens, u?.inputTokens);
  const outTok = pick(u?.output_tokens, u?.outputTokens);
  const reasonTok = pick(u?.reasoning_tokens, u?.reasoningTokens);
  const hasUsage = u !== undefined && (inTok !== null || outTok !== null || reasonTok !== null);
  // 无 usage → 本地估算 chars/4，必须标「估算」（design/01 §6.3）
  const estOut = Math.ceil(JSON.stringify(response).length / 4);

  const calls = typeof response.upstream_calls === 'number' ? response.upstream_calls : 1;
  const latency =
    typeof response.latency_ms === 'number' ? response.latency_ms : measuredLatencyMs;
  const cost = response.cost_usd;

  const cell = (label: string, value: string) => (
    <span className="inline-flex items-baseline gap-1">
      <span className="text-inkSubtle">{label}</span>
      <span className="tabular text-ink">{value}</span>
    </span>
  );

  return (
    <div className={'flex flex-col gap-1 font-mono text-[11px] ' + (className ?? '')}>
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        {hasUsage ? (
          <>
            <span className="text-inkSubtle">usage</span>
            {cell('in', inTok !== null ? String(inTok) : '—')}
            {cell('out', outTok !== null ? String(outTok) : '—')}
            {cell('reason', reasonTok !== null ? String(reasonTok) : '—')}
          </>
        ) : (
          <>
            <span className="text-inkSubtle">tokens</span>
            {cell('in', `~${inputTokens}`)}
            {cell('out', `~${estOut}`)}
            <span className="border border-border bg-bg px-1 py-px text-[10px] uppercase tracking-widest text-inkMuted">
              估算
            </span>
          </>
        )}
      </div>
      <div className="flex flex-wrap items-baseline gap-x-3 gap-y-1">
        {cell('upstream_calls', String(calls))}
        {cell('latency', latency !== null ? `${latency}ms` : '—')}
        {cell('cost_usd', typeof cost === 'number' ? fmtCost(cost) : '—')}
      </div>
    </div>
  );
}

/** 分布条 — 中性色阶循环（tokens：语义色仅状态，不用于装饰） */
const BAR_FILLS = ['bg-ink', 'bg-inkMuted', 'bg-inkSubtle', 'bg-border'];

/** choice/score 的 probabilities 分布条 + 图例（design/01 §6.3） */
function ProbBar({
  entries,
  highlight,
}: {
  entries: [string, number][];
  highlight?: string | null;
}) {
  const total = entries.reduce((s, [, v]) => s + Math.max(v, 0), 0);
  const denom = total > 0 ? total : 1;
  return (
    <div className="w-full">
      <div className="flex h-1.5 w-full overflow-hidden border border-border bg-bg" aria-hidden>
        {entries.map(([k, v], i) => (
          <div
            key={k}
            className={BAR_FILLS[i % BAR_FILLS.length]}
            style={{ width: `${(Math.max(v, 0) / denom) * 100}%` }}
            title={`${k} ${fmtNum(v)}`}
          />
        ))}
      </div>
      <div className="mt-1 flex flex-wrap gap-x-3 gap-y-0.5 text-[10px] text-inkSubtle">
        {entries.map(([k, v], i) => (
          <span
            key={k}
            className={
              'inline-flex items-baseline gap-1' + (k === highlight ? ' font-semibold text-ink' : '')
            }
          >
            <span
              className={'inline-block h-2 w-2 border border-border ' + BAR_FILLS[i % BAR_FILLS.length]}
              aria-hidden
            />
            <span>{k}</span>
            <span className="tabular">{fmtNum(v)}</span>
          </span>
        ))}
      </div>
    </div>
  );
}

function AnswerShell({
  qid,
  typeTag,
  value,
  note,
  children,
}: {
  qid: string;
  typeTag?: string;
  value: string;
  note?: string | null;
  children?: React.ReactNode;
}) {
  return (
    <li className="flex flex-col gap-1">
      <div className="flex flex-wrap items-baseline gap-2">
        <span className="text-inkSubtle">{qid}</span>
        <span className="text-inkSubtle">→</span>
        {typeTag && (
          <span className="border border-border px-1 py-px text-[10px] uppercase tracking-widest text-inkSubtle">
            {typeTag}
          </span>
        )}
        <span className="font-semibold text-ink tabular">{value}</span>
        {note && <span className="text-inkSubtle">{note}</span>}
      </div>
      {children}
    </li>
  );
}

function RawAnswerLine({ qid, raw }: { qid: string; raw: unknown }) {
  return (
    <li className="flex flex-wrap items-baseline gap-2">
      <span className="text-inkSubtle">{qid}</span>
      <span className="text-inkSubtle">→</span>
      <pre className="max-w-full overflow-x-auto border border-border bg-bg px-1.5 py-0.5 text-[11px] text-inkMuted">
        {JSON.stringify(raw, null, 2)}
      </pre>
    </li>
  );
}

/** 单条 Answer — 按 `type` 契约分型，防御分支兜底（B4） */
function AnswerRow({ qid, raw }: { qid: string; raw: unknown }) {
  // 形态未知（null / 标量 / 数组）→ 原始 JSON
  if (raw === null || typeof raw !== 'object' || Array.isArray(raw)) {
    return <RawAnswerLine qid={qid} raw={raw} />;
  }
  const a = raw as Record<string, unknown>;
  const type = typeof a.type === 'string' ? a.type : undefined;

  if (type === 'choice') {
    const value = typeof a.choice === 'string' ? a.choice : '—';
    const conf = typeof a.confidence === 'number' ? `conf ${fmtNum(a.confidence)}` : null;
    const probs = toProbEntries(a.probabilities);
    return (
      <AnswerShell qid={qid} typeTag="choice" value={value} note={conf}>
        {probs && <ProbBar entries={probs} highlight={typeof a.choice === 'string' ? a.choice : null} />}
      </AnswerShell>
    );
  }

  if (type === 'score') {
    const score = typeof a.score === 'number' ? a.score : null;
    const conf = typeof a.confidence === 'number' ? `conf ${fmtNum(a.confidence)}` : null;
    const probs = toProbEntries(a.probabilities);
    const highlight =
      probs && score !== null ? (probs.find(([k]) => Number(k) === score)?.[0] ?? null) : null;
    return (
      <AnswerShell
        qid={qid}
        typeTag="score"
        value={score !== null ? fmtNum(score) : '—'}
        note={conf}
      >
        {probs && <ProbBar entries={probs} highlight={highlight} />}
      </AnswerShell>
    );
  }

  if (type === 'noul' || type === 'boolean') {
    // 布尔族（contracts/01 §4）：概率即置信度 — 无 confidence 行。
    // 双键来源标注（design/01 §6.3）：显示双键中存在者；
    // noul_probability 冻结序 `probability > noul` — 双键不一致时标注有效值；
    // 双缺 = 未验到 → `—`（与真 0.0 区分）。
    const prob = typeof a.probability === 'number' ? a.probability : null;
    const noul = typeof a.noul === 'number' ? a.noul : null;
    const value =
      prob !== null && noul !== null
        ? `prob ${fmtNum(prob)} · noul ${fmtNum(noul)}`
        : prob !== null
          ? `prob ${fmtNum(prob)}`
          : noul !== null
            ? `noul ${fmtNum(noul)}`
            : '—';
    const note =
      prob !== null && noul !== null && prob !== noul ? `noul_p ${fmtNum(prob)}` : null;
    return <AnswerShell qid={qid} typeTag={type} value={value} note={note} />;
  }

  if (type !== undefined) {
    // 未知 type → 原始 JSON 降级（防御）
    return <RawAnswerLine qid={qid} raw={raw} />;
  }

  // 无 `type` → 旧扁平形态防御兜底（A2 前；协议变更不崩 UI）
  const probsFlat = toProbEntries(a.probabilities);
  const flatValue = a.choice ?? a.noul ?? a.boolean ?? a.score ?? probsFlat?.[0]?.[0] ?? '—';
  const conf = typeof a.confidence === 'number' ? `conf ${fmtNum(a.confidence)}` : null;
  return (
    <AnswerShell
      qid={qid}
      value={typeof flatValue === 'number' ? fmtNum(flatValue) : String(flatValue)}
      note={conf}
    />
  );
}
