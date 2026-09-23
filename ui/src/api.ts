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

/** GET /v1/models → data 条目（contracts/05 §2 冻结形状，H1 收口直读） */
export interface ModelInfo {
  id: string;
  object: string;
  upstream: string;
}

/** GET /v1/models → 顶层 upstreams 能力条目（contracts/05 §2） */
export interface UpstreamCapability {
  id: string;
  question_types: string[];
  has_confidence: boolean;
  has_usage: boolean;
  noul_via_boolean: boolean;
}

export interface ModelsResponse {
  object: 'list';
  data: ModelInfo[];
  upstreams: UpstreamCapability[];
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

/** GET /health（contracts/05 §2 冻结：恰两键 status/version） */
export interface HealthResponse {
  status: 'ok';
  version: string;
}

/**
 * health 只认 JSON 新形状（H1 收口）：`{"status":"ok","version":"0.1.0"}`。
 * status === "ok" 才算健康；非 JSON / 其余 status 一律抛错
 * （旧纯文本 `jev-switch MVP` 兼容分支已删）。
 */
export async function fetchHealth(): Promise<HealthResponse> {
  const res = await fetch(`${getBase()}/health`);
  if (!res.ok) throw new Error(`health ${res.status}`);
  let body: unknown;
  try {
    body = await res.json();
  } catch {
    throw new Error('health: non-JSON body');
  }
  if (!body || typeof body !== 'object') throw new Error('health: unexpected body');
  const { status, version } = body as { status?: unknown; version?: unknown };
  if (status !== 'ok') throw new Error(`health: status=${String(status)}`);
  return { status: 'ok', version: typeof version === 'string' ? version : '' };
}

/**
 * GET /v1/models — contracts/05 §2 冻结形状直读
 * （`normalizeModels` 兼容桥与旧 `{models:…}` 键已随 H1 删除）。
 */
export async function fetchModels(): Promise<ModelsResponse> {
  const res = await fetch(`${getBase()}/v1/models`);
  if (!res.ok) throw new Error(`models ${res.status}`);
  return (await res.json()) as ModelsResponse;
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