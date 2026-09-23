import { getBase } from '../api';
import { PROVIDERS_FIXTURE } from '../fixtures/providers.mock';
import { ROUTES_FIXTURE } from '../fixtures/routes.mock';

/**
 * Admin API 客户端（contracts/05 §2 形状字面）。
 *
 * mock/真 API 开关 —— 集中在此一处：
 * - **H3 已切真（A7 `254b4ff` 合入）：默认 'live' 直连 daemon admin API**
 * - 回退 mock：构建期 `VITE_ADMIN_MODE=mock`，或运行时 `setAdminMode('mock')`（调试用，fixtures 保留）
 * - 响应永远只有 api_key_masked，UI 无读回明文（契约 04 §2 红线）
 * - A7 接口备注（已核对）：
 *   1. PUT providers 省略/null `api_key` = 保留；空串 = 清除；**整表替换**（未列出者删除）
 *   2. PUT 响应：providers/routes 均 200 回显 masked 全表（与 GET 同形）
 *   3. 环 400 文案 = `路由配置存在环 (cycle): …`（含「环」，下方 catch 已命中）
 */

export type AdminMode = 'mock' | 'live';

let adminMode: AdminMode = import.meta.env.VITE_ADMIN_MODE === 'mock' ? 'mock' : 'live';

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

/** PUT /v1/admin/providers 条目 — api_key 仅写入时携带；省略/null = 保留、空串 = 清除、非空 = 替换（A7 已核对）；整表替换语义见文件头 */
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
 * 在 mock 内存里模拟「外部改了 toml」。
 * 注：冲突检测现役机制 = GET 快照深比较（admin API 暂无 mtime/hash 字段，
 * contracts/04 §3 的 mtime 方案待 API 扩展后切换）。
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

/** PUT → 响应 masked 全表（已与 A7 核对：200 {providers:[…]} 与 GET 同形；A7 无键时 api_key_masked=""，mock 兼容 null——显示层同 falsy 处理） */
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

/* ---------- routes（contracts/03 §2 / contracts/05 GET·PUT /v1/admin/routes） ---------- */

export interface Route {
  left: string;
  match: 'exact' | 'prefix';
  right: string;
  upstream_model?: string;
  priority: number;
  sticky?: 'none' | 'session';
  on_error?: 'next' | 'fail';
}

export interface RoutesResponse {
  routes: Route[];
}

/** 带 400 语义的 admin 错误（环 → cycleEdges 供 UI 标红） */
export class AdminApiError extends Error {
  readonly status: number;
  readonly cycleEdges?: string[];
  constructor(message: string, status: number, cycleEdges?: string[]) {
    super(message);
    this.name = 'AdminApiError';
    this.status = status;
    this.cycleEdges = cycleEdges;
  }
}

/** 边的稳定标识：left => right（UI 假定该对唯一） */
export function edgeKey(left: string, right: string): string {
  return `${left}=>${right}`;
}

/** 归一化：固定键序 + 默认值（保证 JSON 深比较/UNSAVED 判定稳定） */
export function normalizeRoute(r: Route): Route {
  const out: Route = {
    left: r.left,
    match: r.match === 'prefix' ? 'prefix' : 'exact',
    right: r.right,
    priority: typeof r.priority === 'number' && Number.isFinite(r.priority) ? r.priority : 10,
    sticky: r.sticky === 'session' ? 'session' : 'none',
    on_error: r.on_error === 'fail' ? 'fail' : 'next',
  };
  if (typeof r.upstream_model === 'string' && r.upstream_model.length > 0) {
    out.upstream_model = r.upstream_model;
  }
  return out;
}

/** 本地环检（design/01 §6.2：PUT 前检环；服务端 400 时同样返回边键集合） */
export function findCyclicEdgeKeys(routes: Route[]): string[] {
  const cycle = findCycle(routes);
  if (cycle === null) return [];
  const keys: string[] = [];
  for (let i = 0; i < cycle.length; i++) {
    keys.push(edgeKey(cycle[i], cycle[(i + 1) % cycle.length]));
  }
  return keys;
}

function findCycle(routes: Route[]): string[] | null {
  const adj = new Map<string, string[]>();
  for (const r of routes) {
    const arr = adj.get(r.left) ?? [];
    arr.push(r.right);
    adj.set(r.left, arr);
  }
  const WHITE = 0;
  const GREY = 1;
  const BLACK = 2;
  const color = new Map<string, number>();
  const stack: string[] = [];

  const dfs = (u: string): string[] | null => {
    color.set(u, GREY);
    stack.push(u);
    for (const v of adj.get(u) ?? []) {
      const c = color.get(v) ?? WHITE;
      if (c === GREY) return stack.slice(stack.indexOf(v));
      if (c === WHITE) {
        const got = dfs(v);
        if (got !== null) return got;
      }
    }
    stack.pop();
    color.set(u, BLACK);
    return null;
  };

  for (const u of adj.keys()) {
    if ((color.get(u) ?? WHITE) === WHITE) {
      const got = dfs(u);
      if (got !== null) return got;
    }
  }
  return null;
}

/** mock 内存态 */
let mockRoutes: Route[] = ROUTES_FIXTURE.map(normalizeRoute);

export async function listRoutes(): Promise<RoutesResponse> {
  if (adminMode === 'mock') {
    await delay(80);
    return { routes: mockRoutes.map((r) => ({ ...r })) };
  }
  return request<RoutesResponse>('/v1/admin/routes');
}

export async function putRoutes(routes: Route[]): Promise<RoutesResponse> {
  const normalized = routes.map(normalizeRoute);
  // 环 = 400（contracts/05：DAG 含环则 400）— mock 服务端校验，环边键回传供标红
  const cycleEdges = findCyclicEdgeKeys(normalized);
  if (cycleEdges.length > 0) {
    throw new AdminApiError('路由成环，已拒绝写入', 400, cycleEdges);
  }
  if (adminMode === 'mock') {
    await delay(150);
    mockRoutes = normalized.map((r) => ({ ...r }));
    return { routes: mockRoutes.map((r) => ({ ...r })) };
  }
  try {
    return await request<RoutesResponse>('/v1/admin/routes', {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ routes: normalized }),
    });
  } catch (e) {
    // 已对齐 A7 实际文案：`路由配置存在环 (cycle): a -> b -> a`（含「环」→ 命中）；
    // right 非法 400（「既不是已注册 provider…」）不命中 → 按普通失败回滚，语义正确
    const msg = (e as Error).message;
    if (msg.includes('环')) throw new AdminApiError(msg, 400, findCyclicEdgeKeys(normalized));
    throw e;
  }
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
