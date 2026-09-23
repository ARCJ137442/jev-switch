/**
 * Jev-Switch API client — talks to the Rust daemon at `BASE` (derived from `PORT`).
 *
 * Types are aligned with rs/src/protocol.rs (planned per docs/06-MVP-实现计划.md §阶段 2).
 * The Rust endpoint `POST /v1/systemone` accepts `SystemOneRequest` and returns
 * `SystemOneResponse` (defined in rs/src/protocol.rs).
 */

export type QuestionType = 'choice' | 'score' | 'noul' | 'boolean';

export interface ChoiceQuestion {
  type: 'choice';
  instructions: string;
  criteria: Record<string, string>;
}

export interface ScoreQuestion {
  type: 'score';
  instructions: string;
  criteria: string[];
}

export interface NoulQuestion {
  type: 'noul';
  instructions: string;
  criteria: Record<string, string>;
}

export interface BooleanQuestion {
  type: 'boolean';
  instructions: string;
  criteria: Record<string, string>;
}

export type DecisionQuestion = ChoiceQuestion | ScoreQuestion | NoulQuestion | BooleanQuestion;

export interface SystemOneRequest {
  model: string;
  state: Record<string, unknown>;
  questions: Record<string, DecisionQuestion>;
}

export interface SystemOneAnswer {
  /** 判别联合 tag（契约 01 §4）；防御渲染：缺省时走旧扁平逻辑 */
  type?: string;
  choice?: string;
  score?: number;
  noul?: number;
  /** Vercel 方言键（契约 01 §4 — 双键并存时都保留） */
  probability?: number | null;
  boolean?: boolean;
  confidence?: number | null;
  probabilities?: Record<string, number> | null;
}

export interface SystemOneResponse {
  model: string;
  answers: Record<string, SystemOneAnswer>;
  usage?: unknown;
  upstream?: string;
  latency_ms?: number;
}

export interface ModelInfo {
  id: string;
  object: string;
  upstream?: string;
}

export interface ModelsResponse {
  object: 'list';
  data: ModelInfo[];
}

interface RawModelEntry {
  model?: string;
  id?: string;
  object?: string;
  upstream?: string;
}

interface RawModelsResponse {
  models?: RawModelEntry[];
  data?: RawModelEntry[];
  object?: string;
}

/**
 * Normalize the upstream `/v1/models` payload into `ModelsResponse`.
 *
 * The Rust daemon (rs/) currently returns
 *   { "models": [{ "model": "...", "upstream": "..." }, ...], "upstreams": [...] }
 * but the UI was originally written against an OpenAI-style
 *   { "object": "list", "data": [{ "id": "...", "object": "..." }, ...] }
 * shape. Accept either, prefer `data` when present, and normalize
 * `model` → `id` so the rest of the UI can stay shape-agnostic.
 */
function normalizeModels(raw: unknown): ModelsResponse {
  const obj = (raw ?? {}) as RawModelsResponse;
  const rawList = Array.isArray(obj.data)
    ? obj.data
    : Array.isArray(obj.models)
      ? obj.models
      : [];
  const data: ModelInfo[] = [];
  for (const m of rawList) {
    if (!m) continue;
    const id = (m.id ?? m.model ?? '').toString();
    if (!id) continue;
    const out: ModelInfo = {
      id,
      object: (m.object ?? 'model').toString(),
    };
    if (typeof m.upstream === 'string') out.upstream = m.upstream;
    data.push(out);
  }
  return { object: 'list', data };
}

/**
 * 端口与 base 的单一来源（single source of truth）。
 * ui/src 内任何展示/请求端点都必须从这里派生，禁止再写死字面量。
 */
export const PORT = 11435;
export const BASE = `http://127.0.0.1:${PORT}`;

const DEFAULT_BASE = BASE;

export function getBase(): string {
  if (typeof window !== 'undefined') {
    const fromGlobal = (window as unknown as { __JEV_BASE__?: string }).__JEV_BASE__;
    if (typeof fromGlobal === 'string' && fromGlobal.length > 0) return fromGlobal;
  }
  return DEFAULT_BASE;
}

/**
 * health 双兼容（H1 后简化为只认 JSON）：
 * - 新形态（contracts/05）：JSON `{"status":"ok","version":…}` → ok
 * - 旧形态（兼容期）：纯文本 `jev-switch MVP` → ok
 * 其余 200 响应视为异常，避免「随便一个 200 都算健康」。
 */
export async function fetchHealth(): Promise<string> {
  const res = await fetch(`${getBase()}/health`);
  if (!res.ok) throw new Error(`health ${res.status}`);
  const text = await res.text();
  let parsed: unknown = null;
  try {
    parsed = JSON.parse(text);
  } catch {
    parsed = null;
  }
  if (
    parsed !== null &&
    typeof parsed === 'object' &&
    typeof (parsed as { status?: unknown }).status === 'string'
  ) {
    return text;
  }
  if (text.trim() === 'jev-switch MVP') return text;
  throw new Error(`health unexpected body: ${text.slice(0, 80)}`);
}

export async function fetchModels(): Promise<ModelsResponse> {
  const res = await fetch(`${getBase()}/v1/models`);
  if (!res.ok) throw new Error(`models ${res.status}`);
  const raw = await res.json();
  return normalizeModels(raw);
}

export async function postSystemOne(req: SystemOneRequest): Promise<SystemOneResponse> {
  const res = await fetch(`${getBase()}/v1/systemone`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(req),
  });
  const text = await res.text();
  let body: unknown;
  try {
    body = JSON.parse(text);
  } catch {
    throw new Error(`bad JSON (status ${res.status}): ${text.slice(0, 200)}`);
  }
  if (!res.ok) {
    const message =
      body && typeof body === 'object' && 'error' in body
        ? String((body as { error: unknown }).error)
        : `HTTP ${res.status}`;
    throw new Error(message);
  }
  return body as SystemOneResponse;
}

export const DEFAULT_MODEL_OPTIONS = [
  { value: 'laya-english', label: 'laya-english  (Laya)' },
  { value: 'typesafe-ai/jev', label: 'typesafe-ai/jev  (Vercel)' },
];

export const SAMPLE_STATE = JSON.stringify({ test: true, source: 'jev-switch-mvp-ui' }, null, 2);

export const SAMPLE_QUESTIONS = JSON.stringify(
  {
    q: {
      type: 'noul',
      instructions: 'is this a test?',
      criteria: { true: 'yes', false: 'no' },
    },
  },
  null,
  2,
);