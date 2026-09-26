import { getBase } from '../api';
import { t as tI18n } from '../i18n/core';
import { PROVIDERS_FIXTURE } from '../fixtures/providers.mock';
import { ROUTES_FIXTURE } from '../fixtures/routes.mock';
import type { ProviderView } from '../generated/ProviderView';
import type { ProviderInput } from '../generated/ProviderInput';
import type { ProbeResult } from '../generated/ProbeResult';
import { parseProviderTomlValue, stripTomlComment } from './providerTomlValue';
import { getCallerToken } from '../auth/callerSession';

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
export type AdminProvider = Omit<ProviderView, 'models' | 'api_key_masked'> &
  Partial<Pick<ProviderView, 'models'>> & { api_key_masked: ProviderView['api_key_masked'] | null };

export interface ProvidersResponse {
  providers: AdminProvider[];
}

/** PUT /v1/admin/providers 条目 — api_key 仅写入时携带；省略/null = 保留、空串 = 清除、非空 = 替换（A7 已核对）；整表替换语义见文件头 */
export type AdminProviderWrite = Omit<ProviderInput, 'models' | 'api_key' | 'api_key_env'> &
  Partial<Pick<ProviderInput, 'models' | 'api_key' | 'api_key_env'>>;

export type ProbeResponse = ProbeResult;

/* ---------- 通用请求（#43 cloud：admin 会话附带 + 401/403 全局上报） ---------- */

/**
 * #43 admin 会话（双 token 分权之「管理会话」，**与 api.ts 调用 token 分池**）：
 * `POST /v1/admin/login {password}` → `{token, expires_in}` → 存
 * `window.__JEV_ADMIN_SESSION__` + `localStorage['jev_admin_session']`。
 * 后续 admin 请求带 `Authorization: Bearer <会话>`。密码**只进 login 一次**，
 * 绝不落 localStorage（contracts/04：暴露面必须密文 —— 会话即短时凭据）。
 * local 态：服务端不校验，未登录也不带头 → 零打扰（现状不变）。
 */
export function getAdminSession(): string | null {
  if (typeof window !== 'undefined') {
    const fromGlobal = (window as unknown as { __JEV_ADMIN_SESSION__?: string })
      .__JEV_ADMIN_SESSION__;
    if (typeof fromGlobal === 'string' && fromGlobal.length > 0) return fromGlobal;
    try {
      const stored = window.localStorage.getItem('jev_admin_session');
      if (stored && stored.length > 0) return stored;
    } catch {
      // localStorage 不可用 → 视为未登录
    }
  }
  return null;
}

function storeAdminSession(token: string): void {
  if (typeof window !== 'undefined') {
    (window as unknown as { __JEV_ADMIN_SESSION__?: string }).__JEV_ADMIN_SESSION__ = token;
    try {
      window.localStorage.setItem('jev_admin_session', token);
    } catch {
      // 存不下则本次会话仅内存态（刷新后重登）—— 不阻断
    }
  }
}

/** 401/403 全局上报回调（Shell 注册 → 弹登录小窗）。null = 未注册（local 态恒 null 触发不到）。 */
let authErrorHandler: ((status: number) => void) | null = null;

export function setAuthErrorHandler(fn: ((status: number) => void) | null): void {
  authErrorHandler = fn;
}

/**
 * `POST /v1/admin/login` → 成功则存会话 token（不返回给调用方 —— 密码只经此一跳）。
 * 失败抛 `AdminApiError`（401 密码错 / 500 未配置 —— UI 显示 message 即可）。
 */
export async function loginAdmin(password: string): Promise<void> {
  const res = await fetch(`${getBase()}/v1/admin/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ password }),
  });
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
    throw new AdminApiError(message, res.status);
  }
  if (body === null || typeof body !== 'object') {
    throw new AdminApiError(`bad login response (status ${res.status})`, res.status);
  }
  const token = (body as { token?: unknown }).token;
  if (typeof token !== 'string' || token.length === 0) {
    throw new AdminApiError('login response missing token', res.status);
  }
  storeAdminSession(token);
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers);
  const adminSession = getAdminSession();
  const callToken = getCallerToken();
  const session = callToken ?? adminSession;
  if (session) headers.set('Authorization', `Bearer ${session}`);
  const res = await fetch(`${getBase()}${path}`, { ...init, headers });
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
    const requestId = res.headers.get('x-jev-request-id') ?? undefined;
    if ((res.status === 401 || res.status === 403) && !callToken && adminSession) {
      // cloud 态会话缺失/过期 → 全局登录小窗（Shell 注册）；页面照常拿到异常
      authErrorHandler?.(res.status);
      throw new AdminApiError(message, res.status, undefined, requestId);
    }
    throw new AdminApiError(message, res.status, undefined, requestId);
  }
  if (body === null) throw new Error(`bad JSON (status ${res.status})`);
  return body as T;
}

/** Shared authenticated transport for management features and upstream rehearsal. */
export const adminRequest = request;

/* ---------- mock 引擎 ---------- */

const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function cloneProviders(list: AdminProvider[]): AdminProvider[] {
  return list.map((p) => ({ ...p, models: [...(p.models ?? [])] }));
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
    const clearKey = w.api_key === '';
    return {
      id: w.id,
      kind: w.kind,
      base: w.base,
      enabled: w.enabled,
      name: w.name ?? prev?.name,
      account: w.account ?? prev?.account,
      models: [...(w.models ?? prev?.models ?? [])],
      api_key_masked: clearKey ? '' : hasKey ? maskKey(w.api_key as string) : (prev?.api_key_masked ?? null),
      api_key_set: clearKey ? false : hasKey ? true : (prev?.api_key_set ?? false),
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
  readonly requestId?: string;
  constructor(message: string, status: number, cycleEdges?: string[], requestId?: string) {
    super(message);
    this.name = 'AdminApiError';
    this.status = status;
    this.cycleEdges = cycleEdges;
    this.requestId = requestId;
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
    throw new AdminApiError(tI18n('routing.cycle'), 400, cycleEdges);
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

/* ---------- status / mode / listen（块 5 首页 · 类型单源 = ts-rs generated） ---------- */

import type { StatusResponse } from '../generated/StatusResponse';
import type { PutModeBody as GenPutModeBody } from '../generated/PutModeBody';
import type { PutModeResponse } from '../generated/PutModeResponse';
import type { PutListenResponse } from '../generated/PutListenResponse';
import type { PutPasswordResponse } from '../generated/PutPasswordResponse';
import type { RebindInfo } from '../generated/RebindInfo';

/** GET /v1/admin/status 响应别名（7 键冻结 — ts-rs 单源，勿手写字段） */
export type AdminStatus = StatusResponse;
export type ModeRebind = RebindInfo;
export type ModeResponse = PutModeResponse;
export type ListenResponse = PutListenResponse;
export type { PutPasswordResponse };
export type PutModeBody = GenPutModeBody;

/** status fixture — 与既有 adminMode 开关对齐（无第二套开关）；真端点 404 期间兜底，Mode 二进制合入即切真 */
const STATUS_FIXTURE: AdminStatus = {
  mode: 'local',
  bind: '127.0.0.1:11435',
  bind_explicit: false,
  env_override_active: false,
  password_set: false,
  version: '0.1.0',
  uptime_s: 2 * 3600 + 15 * 60,
};

/**
 * GET /v1/admin/status → {status, source}。
 * - adminMode=mock → fixture（source='mock'）
 * - live 且端点 404（Mode 二进制未重建）→ fixture（source='mock'）
 * - live 200 → 真值（source='live'）；401/403 照常上抛触发登录窗
 */
export async function getStatus(): Promise<{ status: AdminStatus; source: 'live' | 'mock' }> {
  if (adminMode === 'mock') {
    await delay(60);
    return { status: { ...STATUS_FIXTURE }, source: 'mock' };
  }
  try {
    const status = await request<AdminStatus>('/v1/admin/status');
    return { status, source: 'live' };
  } catch (e) {
    const msg = (e as Error).message;
    if (msg.includes('404')) return { status: { ...STATUS_FIXTURE }, source: 'mock' };
    throw e;
  }
}

/**
 * PUT /v1/admin/mode —— cloud 激活密码闸 400 文案由服务端给出
 * （含 `admin password required` → 调用方弹设密对话框后带 admin_password 重发）。
 * generated PutModeBody.admin_password 为 `string | null`（非可选）— 统一显式携带。
 */
export async function putMode(body: PutModeBody): Promise<ModeResponse> {
  if (adminMode === 'mock') {
    await delay(120);
    if (body.mode === 'cloud' && !body.admin_password) {
      throw new Error(
        'admin password required to activate cloud: include admin_password in this request, or set JEV_ADMIN_PASSWORD / toml admin_password first',
      );
    }
    return {
      mode: body.mode as StatusResponse['mode'],
      persisted: true,
      env_override_active: false,
      rebind: {
        from: '127.0.0.1:11435',
        to: body.mode === 'cloud' ? '0.0.0.0:11435' : '127.0.0.1:11435',
        ok: true,
        reason: null,
      },
    };
  }
  return request<ModeResponse>('/v1/admin/mode', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });
}

/** PUT /v1/admin/listen —— addr 可为 ip:port 或 "auto"（恢复 mode 成对默认）；失败旧监听保持 */
export async function putListen(addr: string): Promise<ListenResponse> {
  if (adminMode === 'mock') {
    await delay(120);
    return { addr: addr === 'auto' ? '127.0.0.1:11435' : addr, rebound: true, reason: addr === 'auto' ? 'auto' : null };
  }
  return request<ListenResponse>('/v1/admin/listen', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ addr }),
  });
}

/**
 * PUT /v1/admin/password {password} → {updated, env_override_active}。
 * 门外端点（in-handler 鉴权）；**改后全员重登录** — 成功后调用方必须
 * clearAdminSession() 清本地会话存储回登录态（contracts/04：密码不落 UI）。
 */
export async function putPassword(password: string): Promise<PutPasswordResponse> {
  if (adminMode === 'mock') {
    await delay(120);
    return { updated: true, env_override_active: false };
  }
  return request<PutPasswordResponse>('/v1/admin/password', {
    method: 'PUT',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ password }),
  });
}

/** 改密后清 admin 会话（内存 + localStorage）— UI 回登录态 */
export function clearAdminSession(): void {
  if (typeof window !== 'undefined') {
    delete (window as unknown as { __JEV_ADMIN_SESSION__?: string }).__JEV_ADMIN_SESSION__;
    try {
      window.localStorage.removeItem('jev_admin_session');
    } catch {
      /* ignore */
    }
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
 * 支持 name/account/models/api_key_env；结构不合法时收集 errors（不静默半导入）。
 */
export function parseProvidersToml(text: string): TomlParseResult {
  const providers: AdminProviderWrite[] = [];
  const errors: string[] = [];
  let current: AdminProviderWrite | null = null;

  const flush = () => {
    if (current === null) return;
    if (!current.kind) errors.push(tI18n('api.missingKind', { id: current.id }));
    if (!current.base) errors.push(tI18n('api.missingBase', { id: current.id }));
    if (current.kind && current.base) providers.push(current);
    current = null;
  };

  const lines = text.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = stripTomlComment(lines[i]).trim();
    if (!line) continue;

    const section = line.match(/^\[([^\]]+)\]$/);
    if (section) {
      flush();
      const name = section[1].trim();
      const idMatch = name.match(/^providers\.(.+)$/);
      if (!idMatch) {
        errors.push(tI18n('api.badSection', { n: i + 1, name }));
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
      errors.push(tI18n('api.parseLine', { n: i + 1, line: '' }));
      continue;
    }
    if (current === null) {
      errors.push(tI18n('api.kvOutside', { n: i + 1 }));
      continue;
    }
    const key = kv[1];
    let rawValue = kv[2];
    if (key === 'models' && rawValue.trim().startsWith('[')) {
      while (!rawValue.trim().endsWith(']') && i + 1 < lines.length) rawValue += `\n${stripTomlComment(lines[++i])}`;
    }
    const value = parseProviderTomlValue(rawValue);
    const validType = key === 'enabled' ? typeof value === 'boolean' : key === 'models' ? Array.isArray(value) : typeof value === 'string';
    if (value === null || !validType) {
      errors.push(tI18n('api.valueType', { n: i + 1, key }));
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
        current.api_key_env = value as string;
        break;
      case 'name':
        current.name = value as string;
        break;
      case 'account':
        current.account = value as string;
        break;
      case 'models':
        current.models = [...new Set(value as string[])];
        break;
      default:
        // 未知键不阻断（前向兼容），仅记录
        break;
    }
  }
  flush();
  return { providers, errors };
}
