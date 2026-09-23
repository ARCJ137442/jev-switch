/**
 * Jev-Switch API client — talks to the Rust daemon at http://127.0.0.1:11435
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
  choice?: string;
  score?: number;
  noul?: number;
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

export interface ProviderInfo {
  id: 'vercel' | 'laya';
  label: string;
  enabled: boolean;
  base: string;
  description: string;
}

const DEFAULT_BASE = 'http://127.0.0.1:11435';

function getBase(): string {
  if (typeof window !== 'undefined') {
    const fromGlobal = (window as unknown as { __JEV_BASE__?: string }).__JEV_BASE__;
    if (typeof fromGlobal === 'string' && fromGlobal.length > 0) return fromGlobal;
  }
  return DEFAULT_BASE;
}

export async function fetchHealth(): Promise<string> {
  const res = await fetch(`${getBase()}/health`);
  if (!res.ok) throw new Error(`health ${res.status}`);
  return res.text();
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

export const PROVIDERS: ProviderInfo[] = [
  {
    id: 'vercel',
    label: 'Vercel',
    enabled: true,
    base: 'https://ai-gateway.vercel.sh/v4/ai/evaluation-model',
    description: 'Vercel AI Gateway — noul↔boolean translation layer',
  },
  {
    id: 'laya',
    label: 'Laya',
    enabled: true,
    base: 'http://127.0.0.1:18765/v1/systemone',
    description: 'Local Laya daemon — pass-through Jev protocol',
  },
];

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