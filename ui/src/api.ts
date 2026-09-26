/**
 * Jev-Switch API client — talks to the Rust daemon at `BASE` (derived from `PORT`).
 *
 * 协议类型单一来源 = ts-rs 生成物（B5 收口；contracts/05 §4）：
 * 下列类型全部从 `./generated/*` re-export，禁止再手写重复。
 * 重新生成由 A 线负责（`cargo test --features ts-rs`），本文件只读消费。
 * 本文件保留手写层：端口/base 派生、fetch* 函数体、SystemOneError、示例常量。
 */

/* ══════════════════════════════════════════════════════════════════
   协议类型 — ui/src/generated（ts-rs 生成，contracts/01/05 字面）
   import 供本文件函数层使用；export 供全应用从 `../api` 单点取用。
   ══════════════════════════════════════════════════════════════════ */

import type { Answer } from './generated/Answer';
import type { Criteria } from './generated/Criteria';
import type { ErrorBody } from './generated/ErrorBody';
import type { HealthBody } from './generated/HealthBody';
import type { JevRequest } from './generated/JevRequest';
import type { JevResponse } from './generated/JevResponse';
import type { ModelEntry } from './generated/ModelEntry';
import type { ModelsResponse } from './generated/ModelsResponse';
import type { NoulAnswer } from './generated/NoulAnswer';
import type { NoulKind } from './generated/NoulKind';
import type { Question } from './generated/Question';
import type { UpstreamCapabilityEntry } from './generated/UpstreamCapabilityEntry';
import type { Usage } from './generated/Usage';
import { getCallerToken as getMemoryCallerToken } from './auth/callerSession';

export type {
  Answer,
  Criteria,
  ErrorBody,
  HealthBody,
  JevRequest,
  JevResponse,
  ModelEntry,
  ModelsResponse,
  NoulAnswer,
  NoulKind,
  Question,
  UpstreamCapabilityEntry,
  Usage,
};

/**
 * 端口与 base 的单一来源（single source of truth）。
 * ui/src 内任何展示/请求端点都必须从这里派生，禁止再写死字面量。
 */
import { resolveApiBase } from './api/base';

export const PORT = 11435;
export const BASE = `http://127.0.0.1:${PORT}`;

const DEFAULT_BASE = BASE;

export function getBase(): string {
  if (typeof window !== 'undefined') {
    const fromGlobal = (window as unknown as { __JEV_BASE__?: string }).__JEV_BASE__;
    // 正式构建始终随托管源走，不能以11435端口猜测是否由daemon提供。
    // Vite开发环境仍默认指向本机daemon；显式运行时override优先。
    return resolveApiBase(window.location.href, Boolean(import.meta.env?.DEV), fromGlobal, DEFAULT_BASE);
  }
  return DEFAULT_BASE;
}

/* ══════════════════════════════════════════════════════════════════
   #43 cloud 态调用 token（双 token 分权之「/v1 调用 token」）：
   有则附带 `Authorization: Bearer …`，无则一个头都不加 —— **local 态零打扰**
   （不设 token 的现状请求逐字节不变）。
   Call token is supplied by the in-memory AuthContext; it is never persisted.
   admin 会话是另一枚 token，见 api/admin.ts（不共用）。
   ══════════════════════════════════════════════════════════════════ */

export function getCallToken(): string | null {
  if (typeof window !== 'undefined' && !legacyCallTokenCleanupDone) {
    try {
      // Remove the legacy persisted caller credential; new caller sessions are memory-only.
      window.localStorage.removeItem('jev_token');
    } catch {
      // Storage may be unavailable in private contexts.
    }
    legacyCallTokenCleanupDone = true;
  }
  return getMemoryCallerToken();
}

let legacyCallTokenCleanupDone = false;

/** 有调用 token 才出 `Authorization` 头（cloud）；local 态恒为空对象。 */
function callAuthHeaders(): Record<string, string> {
  const token = getCallToken();
  return token ? { Authorization: `Bearer ${token}` } : {};
}

/**
 * Verify the public daemon identity and endpoint-gateway API revision.
 * A listener returning HTTP 200 alone may be an old demo or another application.
 */
export async function fetchHealth(): Promise<HealthBody> {
  const res = await fetch(`${getBase()}/health`, { headers: callAuthHeaders() });
  if (!res.ok) throw new Error(`health ${res.status}`);
  let body: unknown;
  try {
    body = await res.json();
  } catch {
    throw new Error('health: non-JSON body');
  }
  if (!body || typeof body !== 'object') throw new Error('health: unexpected body');
  const { status, version, product, api_revision, build_revision } = body as Partial<HealthBody>;
  if (status !== 'ok') throw new Error(`health: status=${String(status)}`);
  if (product !== 'jev-switch' || api_revision !== 1 || typeof version !== 'string') {
    throw new Error('health: incompatible or unrecognized Jev-Switch daemon');
  }
  return { status, version, product, api_revision, build_revision: typeof build_revision === 'string' ? build_revision : null };
}

/**
 * GET /v1/models — contracts/05 §2 冻结形状直读（生成类型 `ModelsResponse`）。
 * （`normalizeModels` 兼容桥与旧 `{models:…}` 键已随 H1 删除）。
 */
export async function fetchModels(): Promise<ModelsResponse> {
  const res = await fetch(`${getBase()}/v1/models`, { headers: callAuthHeaders() });
  if (!res.ok) throw new Error(`models ${res.status}`);
  return (await res.json()) as ModelsResponse;
}

/**
 * `/v1/systemone` 错误 — 携带 status + 生成类型 `ErrorBody` 三字段
 * （contracts/05 §3），供 UI 按 design/01 §7 分型文案（422 capability / 503 retryable）。
 */
export class SystemOneError extends Error {
  readonly status: number;
  readonly upstream: string | null;
  readonly retryable: boolean;
  readonly requestId: string | null;
  constructor(
    message: string,
    status: number,
    upstream: string | null = null,
    retryable = false,
    requestId: string | null = null,
  ) {
    super(message);
    this.name = 'SystemOneError';
    this.status = status;
    this.upstream = upstream;
    this.retryable = retryable;
    this.requestId = requestId;
  }
}

export async function postSystemOne(req: JevRequest, signal?: AbortSignal): Promise<JevResponse> {
  const res = await fetch(`${getBase()}/v1/systemone`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...callAuthHeaders() },
    body: JSON.stringify(req),
    signal,
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
          res.headers.get('x-jev-request-id'),
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
