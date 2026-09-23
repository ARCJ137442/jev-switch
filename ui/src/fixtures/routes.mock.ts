import type { Route } from '../api/admin';

/**
 * `[[routes]]` mock fixture — 按 contracts/03 §2 字面四条样例
 * （wire 省略的默认字段在此显式归一：sticky=none / on_error=next）。
 */
export const ROUTES_FIXTURE: Route[] = [
  {
    left: 'jev',
    match: 'exact',
    right: 'jev-fast',
    priority: 5,
    sticky: 'none',
    on_error: 'next',
  },
  {
    left: 'jev',
    match: 'exact',
    right: 'vercel',
    upstream_model: 'typesafe-ai/jev',
    priority: 10,
    sticky: 'session',
    on_error: 'next',
  },
  {
    left: 'jev',
    match: 'exact',
    right: 'laya',
    upstream_model: 'laya-english',
    priority: 30,
    sticky: 'none',
    on_error: 'next',
  },
  {
    left: 'local/*',
    match: 'prefix',
    right: 'laya',
    priority: 40,
    sticky: 'none',
    on_error: 'next',
  },
];

/** 左列对外 model id 集合（routes.left 的并集之外的预置模型） */
export const MODELS_FIXTURE: ReadonlyArray<{ id: string }> = [{ id: 'jev' }, { id: 'local/*' }];

/** 空态展示的示例 toml 片段（design/01 §6.2） */
export const ROUTES_EXAMPLE_TOML = `[[routes]]
left  = "jev"
match = "exact"
right = "vercel"
upstream_model = "typesafe-ai/jev"
priority = 10
sticky = "session"
on_error = "next"

[[routes]]
left  = "jev"
match = "exact"
right = "laya"
upstream_model = "laya-english"
priority = 30`;
