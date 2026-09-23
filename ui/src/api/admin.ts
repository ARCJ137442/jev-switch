import { getBase } from '../api';
import { PROVIDERS_FIXTURE } from '../fixtures/providers.mock';

/**
 * Admin API 客户端（contracts/05 §2 形状字面）。
 *
 * mock/真 API 开关 —— 集中在此一处：
 * - DEV 默认 mock（B2/B3 mock-first；A7 admin 未上线也能全功能开发）
 * - H3（A7 合入）：把下行 DEV 分支改为 'live'，或运行时 setAdminMode('live')
 * - 响应永远只有 api_key_masked，UI 无读回明文（契约 04 §2）
 */

export type AdminMode = 'mock' | 'live';

let adminMode: AdminMode = import.meta.env.DEV ? 'mock' : 'live';

export function getAdminMode(): AdminMode {
  return adminMode;
}

export function setAdminMode(mode: AdminMode): void {
  adminMode = mode;
}

/* ---------- 类型（contracts/05 §2） ---------- */

/** GET /v1/admin/providers 条目 — 禁止 api_key 明文字段 */
export interface AdminProvider {
  id: string;
  kind: string;
  base: string;
  enabled: boolean;
  api_key_masked: string | null;
  api_key_set: boolean;
}

export interface ProvidersResponse {
  providers: AdminProvider[];
}

/** PUT /v1/admin/providers 条目 — api_key 仅写入时携带；省略 = 保留原密钥（H3 与 A7 核对省略语义） */
export interface AdminProviderWrite {
  id: string;
  kind: string;
  base: string;
  enabled: boolean;
  api_key?: string;
}

export interface ProbeResponse {
  ok: boolean;
  latency_ms: number;
  status: number | null;
  error: string | null;
}

/* ---------- 通用请求 ---------- */

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${getBase()}${path}`, init);
  const text = await res.text();
  let body: unknown = null;
  try {
    body = JSON.parse(text);
  } catch {
    body = null;
  }
  if (!res.ok) {
    const message =
      body !== null && typeof body === 'object' && 'error' in body
        ? String((body as { error: unknown }).error)
        : `HTTP ${res.status}`;
    throw new Error(message);
  }
  if (body === null) throw new Error(`bad JSON (status ${res.status})`);
  return body as T;
}

/* ---------- mock 引擎 ---------- */

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function cloneProviders(list: AdminProvider[]): AdminProvider[] {
  return list.map((p) => ({ ...p }));
}

/** mock 内存态（PUT 整表替换语义，写入即刷新） */
let mockProviders: AdminProvider[] = cloneProviders(PROVIDERS_FIXTURE.providers);

/** 契约 04 redact 形态：前缀 3 字符 + **** + 末 4 */
function maskKey(key: string): string {
  if (key.length <= 8) return `****${key.slice(-4)}`;
  return `${key.slice(0, 3)}****${key.slice(-4)}`;
}

const MOCK_PROBE_LATENCY: Record<string, number> = { vercel: 42, laya: 18 };

/**
 * 仅供 dev 验证冲突横幅（GET 深比较驱动）：
 * 在 mock 内存里模拟「外部改了 toml」。H3 后此路径由真 toml mtime 承担。
 */
export function mutateMockExternally(): void {
  const n = mockProviders.length + 1;
  mockProviders = [
    ...cloneProviders(mockProviders),
    {
      id: `external-${n}`,
      kind: 'external',
      base: 'http://127.0.0.1:1/invalid',
      enabled: false,
      api_key_masked: null,
      api_key_set: false,
    },
  ];
}

function assertProvidersShape(list: AdminProviderWrite[]): void {
  if (!Array.isArray(list)) throw new Error('bad request');
  for (const p of list) {
    if (!p || typeof p.id !== 'string' || p.id.length === 0) {
      const err = new Error('provider id required');
      (err as Error & { status: number }).status = 400;
      throw err;
    }
  }
}

/** PUT → 响应 masked 全表（形状同 GET；H3 与 A7 核对） */
function mockPut(list: AdminProviderWrite[]): ProvidersResponse {
  mockProviders = list.map((w) => {
    const prev = mockProviders.find((x) => x.id === w.id);
    const hasKey = typeof w.api_key === 'string' && w.api_key.length > 0;
    return {
      id: w.id,
      kind: w.kind,
      base: w.base,
      enabled: w.enabled,
      api_key_masked: hasKey ? maskKey(w.api_key as string) : (prev?.api_key_masked ?? null),
      api_key_set: hasKey ? true : (prev?.api_key_set ?? false),
    };
  });
  return { providers: cloneProviders(mockProviders) };
}

/* ---------- 端点 ---------- */

export async function listProviders(): Promise<ProvidersResponse> {
  if (adminMode === 'mock') {
    await delay(80);
    return { providers: cloneProviders(mockProviders) };
  }
  return request<ProvidersResponse>('/v1/admin/providers');
}

export async function putProviders(list: AdminProviderWrite[]): Promise<ProvidersResponse> {
  assertProvidersShape(list);
  if (adminMode === 'mock') {
    await delay(150);
    return mockPut(list);
  }
  return request<ProvidersResponse>('/v1/admin/providers', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ providers: list }),
  });
}

export async function probeProvider(id: string): Promise<ProbeResponse> {
  if (adminMode === 'mock') {
    const latency = MOCK_PROBE_LATENCY[id] ?? 33;
    await delay(180);
    if (!mockProviders.some((p) => p.id === id)) {
      return { ok: false, latency_ms: 0, status: 404, error: 'not found' };
    }
    return { ok: true, latency_ms: latency, status: 200, error: null };
  }
  const encoded = encodeURIComponent(id);
  return request<ProbeResponse>(`/v1/admin/providers/${encoded}/probe`, { method: 'POST' });
}

/* ---------- toml 片段解析（贴 toml Tab） ---------- */

export interface TomlParseResult {
  providers: AdminProviderWrite[];
  errors: string[];
}

/**
 * 解析 contracts/04 §6 形态的 `[providers.<id>]` 表片段：
 *   [providers.vercel]
 *   kind = "vercel-gateway"
 *   base = "https://…"
 *   api_key = "…"      # 仅在内存中流转，随 PUT 一次发出
 *   enabled = true
 * 忽略 api_key_env 等未知键；结构不合法时收集 errors（不静默半导入）。
 */
export function parseProvidersToml(text: string): TomlParseResult {
  const providers: AdminProviderWrite[] = [];
  const errors: string[] = [];
  let current: AdminProviderWrite | null = null;

  const stripComment = (line: string): string => {
    let inQuote = false;
    let quoteChar = '';
    for (let i = 0; i < line.length; i++) {
      const ch = line[i];
      if (inQuote) {
        if (ch === quoteChar) inQuote = false;
      } else if (ch === '"' || ch === "'") {
        inQuote = true;
        quoteChar = ch;
      } else if (ch === '#') {
        return line.slice(0, i);
      }
    }
    return line;
  };

  const parseValue = (raw: string): string | boolean | null => {
    const v = raw.trim();
    if ((v.startsWith('"') && v.endsWith('"') && v.length >= 2) ||
        (v.startsWith("'") && v.endsWith("'") && v.length >= 2)) {
      return v.slice(1, -1);
    }
    if (v === 'true') return true;
    if (v === 'false') return false;
    return null;
  };

  const flush = () => {
    if (current === null) return;
    if (!current.kind) errors.push(`[${current.id}] 缺少 kind`);
    if (!current.base) errors.push(`[${current.id}] 缺少 base`);
    if (current.kind && current.base) providers.push(current);
    current = null;
  };

  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = stripComment(lines[i]).trim();
    if (!line) continue;

    const section = line.match(/^\[([^\]]+)\]$/);
    if (section) {
      flush();
      const name = section[1].trim();
      const idMatch = name.match(/^providers\.(.+)$/);
      if (!idMatch) {
        errors.push(`第 ${i + 1} 行：不支持的段 [${name}]（期待 [providers.<id>]）`);
        current = null;
        continue;
      }
      let id = idMatch[1].trim();
      if ((id.startsWith('"') && id.endsWith('"')) || (id.startsWith("'") && id.endsWith("'"))) {
        id = id.slice(1, -1);
      }
      current = { id, kind: '', base: '', enabled: true };
      continue;
    }

    const kv = line.match(/^([A-Za-z0-9_-]+)\s*=\s*(.+)$/);
    if (!kv) {
      errors.push(`第 ${i + 1} 行：无法解析 "${line.slice(0, 40)}"`);
      continue;
    }
    if (current === null) {
      errors.push(`第 ${i + 1} 行：键值对出现在 [providers.<id>] 段之外`);
      continue;
    }
    const key = kv[1];
    const value = parseValue(kv[2]);
    if (value === null) {
      errors.push(`第 ${i + 1} 行：${key} 仅支持引号字符串或布尔值`);
      continue;
    }
    switch (key) {
      case 'kind':
        current.kind = String(value);
        break;
      case 'base':
        current.base = String(value);
        break;
      case 'enabled':
        current.enabled = value === true;
        break;
      case 'api_key':
        current.api_key = String(value);
        break;
      case 'api_key_env':
        // 兼容 providers.example.toml；环境变量解析属 daemon 职责，UI 忽略
        break;
      default:
        // 未知键不阻断（前向兼容），仅记录
        break;
    }
  }
  flush();
  return { providers, errors };
}
