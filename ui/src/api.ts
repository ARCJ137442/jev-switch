/**
 * Jev-Switch API client — talks to the Rust daemon at `BASE` (derived from `PORT`).
 *
 * TODO(B5): 以 ui/src/generated 生成类型替换（ts-rs，contracts/05 §4）。
 * 本文件协议类型为 contracts/01 手写临时版，逐字段对齐
 * `rs/crates/jev-protocol`（A2 `fc91ffa`）。
 * `POST /v1/systemone` 收 `JevRequest`，回 `JevResponse`。
 */

/* ══════════════════════════════════════════════════════════════════
   协议类型（contracts/01 — B4 手写版）
   TODO(B5): 以 ui/src/generated 生成类型替换
   ══════════════════════════════════════════════════════════════════ */

/** 对外判别值仅三变体（§1：`boolean` 不是合法输入，收下按 noul 归一） */
export type QuestionType = 'choice' | 'score' | 'noul';

export interface ChoiceQuestion {
  type: 'choice';
  instructions: string;
  /** Map 形态（§3） */
  criteria: Record<string, string>;
}

export interface ScoreQuestion {
  type: 'score';
  instructions: string;
  /** 有序档位 List（§3） */
  criteria: string[];
}

export interface NoulQuestion {
  type: 'noul';
  instructions: string;
  /** Bool 恰两键（§3） */
  criteria: { true: string; false: string };
}

/** Question 判别联合（§3）。入站 `boolean` 由后端归一为 noul（§1） */
export type Question = ChoiceQuestion | ScoreQuestion | NoulQuestion;

/** criteria 三形态（§3）：Map（choice）/ List（score）/ Bool（noul） */
export type Criteria =
  | Record<string, string>
  | string[]
  | { true: string; false: string };

/** JevRequest（§2） */
export interface JevRequest {
  model: string;
  /** 必填；任意 JSON — 允许 null / 标量 / 对象（§2） */
  state: unknown;
  questions: Record<string, Question>;
}

/* ── Answer（§4 判别联合，`type` 必有） ── */

export interface ChoiceAnswer {
  type: 'choice';
  choice: string;
  /** 必填，完整分布 */
  probabilities: Record<string, number>;
  /** 分布集中度，≠ 最高项概率 */
  confidence: number;
}

export interface ScoreAnswer {
  type: 'score';
  score: number;
  probabilities: Record<string, number>;
  confidence: number;
}

/**
 * 布尔族（§4）：概率本身就是置信度 — **无 confidence 字段**。
 * wire `type` 由 Rust `NoulAnswer.kind` rename 而来（"noul" | "boolean"）；
 * 请求侧写出恒 `"noul"`（§6），回答侧保留上游方言（Vercel 可回 `"boolean"`）。
 * 双键并存时都保留（§6）；取值顺序冻结 `probability > noul`（noul_probability）。
 * 双缺 = 未验到（≠ 0.0，渲染须能区分）。
 */
export interface NoulAnswer {
  type: 'noul' | 'boolean';
  /** 官方 / OpenRouter 键 */
  noul?: number | null;
  /** Vercel 键 — 必须保留，禁止吞掉 */
  probability?: number | null;
}

export type Answer = ChoiceAnswer | ScoreAnswer | NoulAnswer;

/** token 用量（§5）：读容忍 camel / snake 双拼写，写出统一 snake（显示用 snake） */
export interface Usage {
  input_tokens?: number | null;
  output_tokens?: number | null;
  reasoning_tokens?: number | null;
  /** camel 别名（仅读容忍；写出恒 snake） */
  inputTokens?: number | null;
  outputTokens?: number | null;
  reasoningTokens?: number | null;
}

/**
 * JevResponse（§5）。
 * - `model?` 可缺省；`usage?` 可缺省
 * - `upstream_calls?` 缺省按 1；`latency_ms?`；`cost_usd` null ≠ 0（未知显示 `—`）
 * - `extra` flatten：providerMetadata 等未知顶层键在此。
 *   **无正式 `upstream` 字段** — 后端不产出；旧 `upstream?: string` 已删
 *   （顶层同名键只会作为 extra 以 unknown 透传）。
 */
export interface JevResponse {
  model?: string;
  answers: Record<string, Answer>;
  usage?: Usage;
  upstream_calls?: number;
  latency_ms?: number;
  cost_usd?: number | null;
  /** flatten extra（contracts/01 §5）— providerMetadata 等 */
  [extra: string]: unknown;
}

/** @deprecated 过渡别名 — 主名 `JevRequest`（contracts/01；rs 同款 alias） */
export type SystemOneRequest = JevRequest;
/** @deprecated 过渡别名 — 主名 `JevResponse`（contracts/01；rs 同款 alias） */
export type SystemOneResponse = JevResponse;

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

/** ErrorBody（contracts/05 §3 统一错误体，redact 后透出） */
export interface ErrorBody {
  error: string;
  upstream?: string | null;
  retryable?: boolean;
}

/**
 * `/v1/systemone` 错误 — 携带 status + ErrorBody 三字段，
 * 供 UI 按 design/01 §7 分型文案（422 capability / 503 retryable）。
 */
export class SystemOneError extends Error {
  readonly status: number;
  readonly upstream: string | null;
  readonly retryable: boolean;
  constructor(
    message: string,
    status: number,
    upstream: string | null = null,
    retryable = false,
  ) {
    super(message);
    this.name = 'SystemOneError';
    this.status = status;
    this.upstream = upstream;
    this.retryable = retryable;
  }
}

export async function postSystemOne(req: JevRequest): Promise<JevResponse> {
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
    if (body && typeof body === 'object' && 'error' in body) {
      const eb = body as Partial<ErrorBody>;
      if (typeof eb.error === 'string') {
        throw new SystemOneError(
          eb.error,
          res.status,
          typeof eb.upstream === 'string' ? eb.upstream : null,
          eb.retryable === true,
        );
      }
    }
    throw new Error(`HTTP ${res.status}`);
  }
  return body as JevResponse;
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