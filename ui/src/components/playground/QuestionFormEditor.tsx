import { useEffect, useState } from 'react';

/**
 * 结构化题目表单编辑器（验收反馈 UI-3 / UI-4，参考 jevplayground.com 呈现）：
 * - 每题一个卡片框（instructions 独立文本域）+ Decision type 分段切换（choice/score/noul）
 * - choice：编号行 [key][描述] ×删除，`+ Add choice`（≤10，显示 n of 10）
 * - score：有序档位行（↑↓ 调序 + 删除），`+ Add level`
 * - noul：固定 true / false 两行描述（契约 Bool 形态）
 * - 真·多问题：`+ Add question` 增题、可改 qid、可删（至少保留 1 题）
 * - 与 questionsJson 双向同步：表单每次变更写回 JSON（Run/chips/计数的唯一真值仍是 JSON 文本）
 * - JSON 无效时保留 last-good 表单并显示错误条（在 JSON 视图修复）
 *
 * 契约边界：仅 choice/score/noul（contracts/01 §1，无 boolean）；
 * instructions 必填非空由服务端 400 兜底，表单侧给 required 视觉提示。
 */

interface QEntry {
  key: string; // choice: criteria 键 / noul: 'true'|'false'（固定）/ score: 未用
  value: string;
}

interface QItem {
  qid: string;
  type: 'choice' | 'score' | 'noul';
  instructions: string;
  entries: QEntry[];
}

interface Props {
  questionsJson: string;
  onChange: (json: string) => void;
}

const TYPE_META: ReadonlyArray<{ id: QItem['type']; label: string; title: string }> = [
  { id: 'choice', label: 'Choice', title: '从选项中选一个标签并返回各自概率' },
  { id: 'score', label: 'Score', title: '按有序档位打一个分数' },
  { id: 'noul', label: 'Noul', title: '是非题：返回 0-1 概率（true/false 两行描述）' },
];

const MAX_CHOICES = 10;

/* ---------- 解析 / 序列化 ---------- */

function str(v: unknown): string {
  return typeof v === 'string' ? v : '';
}

function fromPlain(obj: unknown): QItem[] {
  if (!obj || typeof obj !== 'object' || Array.isArray(obj)) {
    throw new Error('questions 必须是对象 { qid: {...} }');
  }
  const items: QItem[] = [];
  for (const [qid, raw] of Object.entries(obj as Record<string, unknown>)) {
    if (!raw || typeof raw !== 'object') {
      throw new Error(`题目 "${qid}" 不是对象`);
    }
    const q = raw as Record<string, unknown>;
    const type = str(q.type);
    if (type !== 'choice' && type !== 'score' && type !== 'noul') {
      throw new Error(`题目 "${qid}" 的 type="${type || '?'}" 不受支持（仅 choice/score/noul）— 请在 JSON 视图编辑`);
    }
    const instructions = str(q.instructions);
    const crit = q.criteria;
    let entries: QEntry[];
    if (type === 'choice') {
      if (crit && typeof crit === 'object' && !Array.isArray(crit)) {
        entries = Object.entries(crit as Record<string, unknown>).map(([k, v]) => ({
          key: k,
          value: str(v),
        }));
      } else if (Array.isArray(crit)) {
        entries = crit.map((v, i) => ({ key: `opt${i + 1}`, value: str(v) }));
      } else {
        entries = [{ key: 'a', value: '' }];
      }
    } else if (type === 'score') {
      if (Array.isArray(crit)) {
        entries = crit.map((v) => ({ key: '', value: str(v) }));
      } else if (crit && typeof crit === 'object') {
        entries = Object.values(crit as Record<string, unknown>).map((v) => ({
          key: '',
          value: str(v),
        }));
      } else {
        entries = [{ key: '', value: '' }];
      }
    } else {
      // noul — Bool 形态 {true, false}（缺失补空行）
      const b = (crit ?? {}) as Record<string, unknown>;
      entries = [
        { key: 'true', value: str(b.true) },
        { key: 'false', value: str(b.false) },
      ];
    }
    items.push({ qid, type, instructions, entries });
  }
  if (items.length === 0) {
    throw new Error('questions 为空 — 至少需要 1 题');
  }
  return items;
}

function toPlain(items: QItem[]): { json?: string; error?: string } {
  // qid 校验：非空 + 唯一（契约 Record 键）
  const seen = new Set<string>();
  for (const it of items) {
    const id = it.qid.trim();
    if (!id) return { error: 'qid 不能为空' };
    if (seen.has(id)) return { error: `qid 重复："${id}" — 请改名` };
    seen.add(id);
  }
  const out: Record<string, unknown> = {};
  for (const it of items) {
    if (it.type === 'choice') {
      const criteria: Record<string, string> = {};
      for (const e of it.entries) {
        criteria[e.key] = e.value;
      }
      out[it.qid.trim()] = { type: 'choice', instructions: it.instructions, criteria };
    } else if (it.type === 'score') {
      out[it.qid.trim()] = {
        type: 'score',
        instructions: it.instructions,
        criteria: it.entries.map((e) => e.value),
      };
    } else {
      out[it.qid.trim()] = {
        type: 'noul',
        instructions: it.instructions,
        criteria: { true: it.entries[0]?.value ?? '', false: it.entries[1]?.value ?? '' },
      };
    }
  }
  return { json: JSON.stringify(out, null, 2) };
}

/** 题型切换时的条目尽力保留（值不丢、按目标形态规整） */
function retargetEntries(entries: QEntry[], to: QItem['type']): QEntry[] {
  if (to === 'noul') {
    return [
      { key: 'true', value: entries[0]?.value ?? '' },
      { key: 'false', value: entries[1]?.value ?? '' },
    ];
  }
  if (to === 'score') {
    const vals = entries.filter((e) => e.value !== '');
    return (vals.length > 0 ? vals : [{ key: '', value: '' }]).map((e) => ({
      key: '',
      value: e.value,
    }));
  }
  // choice
  return entries.map((e, i) => ({
    key: e.key && e.key !== 'true' && e.key !== 'false' ? e.key : `opt${i + 1}`,
    value: e.value,
  }));
}

function nextQid(items: QItem[]): string {
  const used = new Set(items.map((i) => i.qid));
  for (let n = 1; n < 100; n++) {
    const cand = `q${n}`;
    if (!used.has(cand)) return cand;
  }
  return `q${Date.now() % 10000}`;
}

/* ---------- 组件 ---------- */

export function QuestionFormEditor({ questionsJson, onChange }: Props) {
  const [items, setItems] = useState<QItem[] | null>(null);
  const [parseErr, setParseErr] = useState<string | null>(null);
  const [pushErr, setPushErr] = useState<string | null>(null);

  // JSON → 表单（外源变更：样例切换 / JSON 视图编辑）
  useEffect(() => {
    try {
      const parsed = fromPlain(JSON.parse(questionsJson || '{}'));
      setItems(parsed);
      setParseErr(null);
      setPushErr(null);
    } catch (e) {
      // 解析失败 → 保留 last-good 表单 + 错误条（切到 JSON 视图修复）
      setParseErr((e as Error).message);
    }
  }, [questionsJson]);

  /** 表单 → JSON（qid 非法时不写回，保留用户正在修的输入） */
  const commit = (next: QItem[]) => {
    setItems(next);
    const { json, error } = toPlain(next);
    if (error) {
      setPushErr(error);
      return;
    }
    setPushErr(null);
    onChange(json as string);
  };

  const patchItem = (idx: number, patch: Partial<QItem>) =>
    commit(items!.map((it, i) => (i === idx ? { ...it, ...patch } : it)));

  const removeItem = (idx: number) => {
    if (items!.length <= 1) return;
    commit(items!.filter((_, i) => i !== idx));
  };

  const addItem = () =>
    commit([...items!, { qid: nextQid(items!), type: 'choice', instructions: '', entries: [
      { key: 'a', value: '' },
      { key: 'b', value: '' },
    ] }]);

  if (items === null) {
    return (
      <div className="border border-border bg-soft px-3 py-2 font-mono text-xs text-inkMuted">
        loading form…
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-3">
      {parseErr && (
        <div
          role="alert"
          className="rounded border border-danger bg-dangerBg px-3 py-2 font-mono text-[10px] uppercase tracking-widest text-danger"
        >
          JSON 无效，下方为最近一次有效表单 — 请切到 JSON 视图修复：{parseErr}
        </div>
      )}
      {pushErr && (
        <div
          role="alert"
          className="rounded border border-warn bg-warnBg px-3 py-2 font-mono text-[10px] uppercase tracking-widest text-warn"
        >
          {pushErr}（暂未写回 JSON）
        </div>
      )}

      {items.map((it, idx) => (
        <QuestionCard
          key={idx}
          item={it}
          index={idx}
          total={items.length}
          onChange={(patch) => patchItem(idx, patch)}
          onRemove={() => removeItem(idx)}
        />
      ))}

      <button
        type="button"
        onClick={addItem}
        aria-label="add question"
        className="h-8 w-full border border-border bg-panel font-mono text-xs text-inkMuted transition-colors hover:border-primaryFill hover:bg-primaryFill hover:text-white"
      >
        + Add question
      </button>
    </div>
  );
}

/* ---------- 单题卡片 ---------- */

interface CardProps {
  item: QItem;
  index: number;
  total: number;
  onChange: (patch: Partial<QItem>) => void;
  onRemove: () => void;
}

function QuestionCard({ item, index, total, onChange, onRemove }: CardProps) {
  const typeSwitch = (t: QItem['type']) =>
    onChange({ type: t, entries: retargetEntries(item.entries, t) });

  const setEntry = (i: number, patch: Partial<QEntry>) =>
    onChange({
      entries: item.entries.map((e, j) => (j === i ? { ...e, ...patch } : e)),
    });

  const removeEntry = (i: number) => onChange({ entries: item.entries.filter((_, j) => j !== i) });

  const moveEntry = (i: number, dir: -1 | 1) => {
    const j = i + dir;
    if (j < 0 || j >= item.entries.length) return;
    const next = [...item.entries];
    [next[i], next[j]] = [next[j], next[i]];
    onChange({ entries: next });
  };

  const addChoice = () => {
    if (item.entries.length >= MAX_CHOICES) return;
    const used = new Set(item.entries.map((e) => e.key));
    let n = item.entries.length + 1;
    let key = `opt${n}`;
    while (used.has(key)) {
      n++;
      key = `opt${n}`;
    }
    onChange({ entries: [...item.entries, { key, value: '' }] });
  };

  const addLevel = () => onChange({ entries: [...item.entries, { key: '', value: '' }] });

  const isNoul = item.type === 'noul';
  const isChoice = item.type === 'choice';

  return (
    <section
      aria-label={`question ${index + 1}: ${item.qid}`}
      className="overflow-hidden rounded-card border border-border bg-panel"
    >
      {/* 卡片头：qid + 题型分段 + 删除 */}
      <header className="flex flex-wrap items-center gap-2 border-b border-border px-3 py-2">
        <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle">
          Q{index + 1}
        </span>
        <input
          value={item.qid}
          onChange={(e) => onChange({ qid: e.target.value })}
          aria-label={`question ${index + 1} id`}
          spellCheck={false}
          className="h-8 w-36 border border-border bg-soft px-2 font-mono text-xs text-ink"
          placeholder="qid"
        />
        <div className="flex items-center gap-1" role="group" aria-label={`decision type for ${item.qid}`}>
          {TYPE_META.map((t) => (
            <button
              key={t.id}
              type="button"
              title={t.title}
              aria-pressed={item.type === t.id}
              onClick={() => typeSwitch(t.id)}
              className={
                'h-8 border px-2.5 font-mono text-xs transition-colors ' +
                (item.type === t.id
                  ? 'border border-primaryFill bg-primaryFill text-white'
                  : 'border-border bg-panel text-inkMuted hover:border-primaryBright hover:text-ink')
              }
            >
              {t.label}
            </button>
          ))}
        </div>
        <span className="ml-auto flex items-center gap-2">
          <span className="font-mono text-[10px] uppercase tracking-widest text-inkSubtle tabular">
            {isChoice ? `${item.entries.length} of ${MAX_CHOICES}` : `${item.entries.length} item(s)`}
          </span>
          <button
            type="button"
            onClick={onRemove}
            disabled={total <= 1}
            aria-label={`delete question ${item.qid}`}
            title={total <= 1 ? '至少保留 1 题' : '删除本题'}
            className="h-8 w-8 border border-border font-mono text-xs text-inkMuted transition-colors hover:border-danger hover:text-danger disabled:cursor-not-allowed disabled:opacity-40"
          >
            ×
          </button>
        </span>
      </header>

      <div className="flex flex-col gap-3 px-3 py-3">
        {/* 题面 — 一个问题一个框 */}
        <label className="flex flex-col gap-1.5">
          <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
            Question
            {item.instructions.trim() === '' && (
              <span className="ml-2 text-danger">required</span>
            )}
          </span>
          <textarea
            value={item.instructions}
            onChange={(e) => onChange({ instructions: e.target.value })}
            rows={2}
            aria-label={`question ${index + 1} instructions`}
            className={
              'w-full resize-y border bg-soft p-2.5 font-mono text-xs leading-relaxed text-ink ' +
              (item.instructions.trim() === '' ? 'border-danger' : 'border-border')
            }
            placeholder="要 Jev 做的决定是什么？/ The decision you want Jev to make."
          />
        </label>

        {/* 题型专属区 */}
        {isNoul ? (
          <div className="flex flex-col gap-2">
            <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
              Criteria (true / false)
            </span>
            {(['true', 'false'] as const).map((k, i) => (
              <div key={k} className="flex items-center gap-2">
                <span className="w-12 shrink-0 font-mono text-xs text-inkSubtle">{k}</span>
                <input
                  value={item.entries[i]?.value ?? ''}
                  onChange={(e) => setEntry(i, { value: e.target.value })}
                  aria-label={`question ${index + 1} ${k} description`}
                  className="h-8 flex-1 border border-border bg-soft px-2 font-mono text-xs text-ink"
                  placeholder={k === 'true' ? 'true 时的含义' : 'false 时的含义'}
                />
              </div>
            ))}
          </div>
        ) : isChoice ? (
          <div className="flex flex-col gap-2">
            <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
              Choices · key = 概率分布键 / 描述 = 展示文案
            </span>
            {item.entries.map((e, i) => (
              <div key={i} className="flex items-center gap-2">
                <span className="w-6 shrink-0 font-mono text-[10px] text-inkSubtle tabular">
                  {String(i + 1).padStart(2, '0')}
                </span>
                <input
                  value={e.key}
                  onChange={(ev) => setEntry(i, { key: ev.target.value })}
                  aria-label={`question ${index + 1} choice ${i + 1} key`}
                  spellCheck={false}
                  className="h-8 w-32 shrink-0 border border-border bg-soft px-2 font-mono text-xs text-ink"
                  placeholder="key"
                />
                <input
                  value={e.value}
                  onChange={(ev) => setEntry(i, { value: ev.target.value })}
                  aria-label={`question ${index + 1} choice ${i + 1} label`}
                  className="h-8 min-w-0 flex-1 border border-border bg-soft px-2 font-mono text-xs text-ink"
                  placeholder="选项描述"
                />
                <button
                  type="button"
                  onClick={() => removeEntry(i)}
                  disabled={item.entries.length <= 1}
                  aria-label={`remove choice ${i + 1}`}
                  className="h-8 w-8 shrink-0 border border-border font-mono text-xs text-inkMuted transition-colors hover:border-danger hover:text-danger disabled:opacity-40"
                >
                  ×
                </button>
              </div>
            ))}
            <button
              type="button"
              onClick={addChoice}
              disabled={item.entries.length >= MAX_CHOICES}
              aria-label={`add choice to question ${index + 1}`}
              className="h-8 border border-border bg-panel font-mono text-xs text-inkMuted transition-colors hover:border-primaryFill hover:bg-primaryFill hover:text-white disabled:cursor-not-allowed disabled:opacity-40"
            >
              + Add choice{item.entries.length >= MAX_CHOICES ? ' (max 10)' : ''}
            </button>
          </div>
        ) : (
          <div className="flex flex-col gap-2">
            <span className="font-mono text-[10px] font-semibold uppercase tracking-widest text-inkSubtle">
              Levels · 有序档位（低 → 高）
            </span>
            {item.entries.map((e, i) => (
              <div key={i} className="flex items-center gap-2">
                <span className="w-6 shrink-0 font-mono text-[10px] text-inkSubtle tabular">
                  {String(i + 1).padStart(2, '0')}
                </span>
                <input
                  value={e.value}
                  onChange={(ev) => setEntry(i, { value: ev.target.value })}
                  aria-label={`question ${index + 1} level ${i + 1}`}
                  className="h-8 min-w-0 flex-1 border border-border bg-soft px-2 font-mono text-xs text-ink"
                  placeholder="档位描述"
                />
                <button
                  type="button"
                  onClick={() => moveEntry(i, -1)}
                  disabled={i === 0}
                  aria-label={`move level ${i + 1} up`}
                  className="h-8 w-8 shrink-0 border border-border font-mono text-xs text-inkMuted transition-colors hover:border-primaryBright hover:text-ink disabled:opacity-40"
                >
                  ↑
                </button>
                <button
                  type="button"
                  onClick={() => moveEntry(i, 1)}
                  disabled={i === item.entries.length - 1}
                  aria-label={`move level ${i + 1} down`}
                  className="h-8 w-8 shrink-0 border border-border font-mono text-xs text-inkMuted transition-colors hover:border-primaryBright hover:text-ink disabled:opacity-40"
                >
                  ↓
                </button>
                <button
                  type="button"
                  onClick={() => removeEntry(i)}
                  disabled={item.entries.length <= 1}
                  aria-label={`remove level ${i + 1}`}
                  className="h-8 w-8 shrink-0 border border-border font-mono text-xs text-inkMuted transition-colors hover:border-danger hover:text-danger disabled:opacity-40"
                >
                  ×
                </button>
              </div>
            ))}
            <button
              type="button"
              onClick={addLevel}
              aria-label={`add level to question ${index + 1}`}
              className="h-8 border border-border bg-panel font-mono text-xs text-inkMuted transition-colors hover:border-primaryFill hover:bg-primaryFill hover:text-white"
            >
              + Add level
            </button>
          </div>
        )}
      </div>
    </section>
  );
}
