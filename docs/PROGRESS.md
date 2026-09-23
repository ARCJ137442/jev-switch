# Jev-Switch 进度快照 — v0.1.0-mvp

> **封存时间**：2026-09-23
> **Git tag**：`v0.1.0-mvp`
> **目标**：MVP（M0）端到端通过 + 0 控制台报错

---

## 一、4 条"做到"标准实测结果

| # | 标准 | 验证方式 | 结果 |
|---|---|---|---|
| 1 | POST {model:"laya-english"} → 真实路由到 Laya | CDP 端到端：`Run Jev ↗` 点击 | ✅ `auth: 0.7608, confidence: 0.4782`（170-200ms） |
| 2 | POST {model:"typesafe-ai/jev"} → 真实路由到 Vercel | curl 模拟浏览器 | ✅ Vercel AI Gateway 真实响应 |
| 3 | 调用方发 `type: noul` → 网关自动翻译为 Vercel `type: boolean` | curl 端到端测试 | ✅ Vercel 收到 boolean + boolean→noul 反向翻译 |
| 4 | 切换 model 字段即可，调用方无需适配格式 | UI Model 下拉切换 + curl | ✅ 同一份 payload 只改 model 字段，自动路由 |

---

## 二、当前可工作的状态

### 三服务（全部 alive）

| 服务 | URL | 后端 |
|---|---|---|
| Laya 本地推理 | `127.0.0.1:18765` | Python `jev_laya_server.py` + Laya CUDA |
| jev-switch Rust | `127.0.0.1:8765` | axum + tokio + reqwest-rustls |
| React UI (Vite) | `127.0.0.1:5173` | React 18 + TS + Tailwind |

### API 端点

```
GET  /health         → "jev-switch MVP"
GET  /v1/models      → 2 models + 2 upstreams with capability matrix
POST /v1/systemone   → 路由到 C1/C2/C5，按 model 字段
OPTIONS *            → CORS 预检（Access-Control-Allow-Origin: *）
```

### 浏览器实测证据

- Edge headless + CDP 验证：`Runtime.exceptionThrown` 0 次
- React root 渲染：`root.children=1, innerHTML.length=12884`
- 实际请求：UI 调 `/v1/models` 拿到 2 models，UI 调 `/v1/systemone {model:"laya-english"}` 拿到 Laya 真实推理响应
- 截图：`H:\A137442\Develop\AI\Jev\Jev-Switch\..\..\jev-final.png`（或 `C:\Users\56506\jev-final.png`）

---

## 三、commit 链（11 个 commits）

```
37b4242 fix(ui): React app 渲染崩溃 root=0 修复
17fe7c6 fix(rust): add CORS layer for browser fetch
c3180ac feat(ui): 重做 React UI（参考 jevplayground.com Linear/Stripe 文档风）
35c5d65 feat(rust): MVP core — protocol types, upstream trait, Vercel + Laya upstream, router, noul→boolean translation, config, axum server
0762ce0 feat(ui): MVP React UI (M0.10)
c99592f docs(06): Jev-Switch MVP (M0) 实现计划
b2a9387 docs(05): 从 jev-life 超越的 5 个设计点
5dd0776 docs(04): 架构设计 — 借鉴 sys1
3ca19ba docs(03): 上游类别与协议兼容矩阵
2baa5e0 docs: initial commit — DeepSeek 原始计划书 + v2.1 新版计划书
```

---

## 四、仓库结构

```
H:\A137442\Develop\AI\Jev\Jev-Switch\        ← GitHub: ARCJ137442/jev-switch (private)
├── docs/                       规划与文档（2141 行）
│   ├── 01-Jev-Switch-原始计划书-DeepSeek.md   baseline
│   ├── 02-Jev-Switch-新版计划书.md           v2.1
│   ├── 03-上游类别与协议兼容矩阵.md           capability 矩阵
│   ├── 04-架构设计-from-sys1-借鉴.md        Rust + React + Tauri
│   ├── 05-从-jev-life-超越的设计点.md        5 个超越能力
│   ├── 06-MVP-实现计划.md                   M0 11 步执行清单
│   ├── PROGRESS.md                          本文件
│   └── README.md
├── rs/                         Rust 后端（2014 行 .rs）
│   ├── Cargo.toml
│   ├── providers.example.toml
│   └── src/  (8 .rs 文件)
└── ui/                         React UI（~270 行 .tsx）
    ├── package.json
    ├── tailwind.config.js
    └── src/  (App.tsx + api.ts + components/)
```

---

## 五、6 个文档对应 6 个设计层次

| 文档 | 答的问题 |
|---|---|
| 01 | DeepSeek 的原始 4 条需求 |
| 02 | 整体规划（5 类上游、桥接模式、M1-M7 切分） |
| 03 | 每个上游支持什么字段（capability 矩阵） |
| 04 | 怎么实现（Rust axum 路由 + React + Tauri） |
| 05 | 比 jev-life 多做了什么（青出于蓝） |
| 06 | M0 这 11 步怎么做 |

---

## 六、关键技术决策（已固化的）

| 决策 | 来源 | 状态 |
|---|---|---|
| Rust 后端 + axum + reqwest-rustls | 04- §2.3 | ✅ 落地 |
| React 前端 + Vite + Tailwind | 06- §阶段 7 | ✅ 落地（subAgent B 实现） |
| Vercel evaluation-model `noul↔boolean` 翻译层 | 03- §2.4 | ✅ 落地（35c5d65） |
| `family=4` undici Agent（IPv4 only） | jev-decision-lab 实测 | 已记入 03- §3.3（**未实装**，MVP 留给用户环境 DNS） |
| config.toml 多上游配置 | 06- §阶段 5 + 02- §3.3 | ✅ 落地（35c5d65） |
| Router 静态 model→upstream 映射 | 06- §阶段 6 | ✅ 落地 |
| CORS `CorsLayer::very_permissive()` | M0 简化 | ✅ 落地（17fe7c6） |
| RootErrorBoundary 兜底 | 37b4242 | ✅ 新加 |

---

## 七、MVP 不做什么（M0 边界）

按 `06-MVP-实现计划.md §一 OUT`：

- ❌ 其他 5 类上游（C1 TypeSafe / C2 OpenRouter / C2 LM Studio / C2 sys1 candle / C3 broker / C4 LLM2Jev）—— 只 C2 Vercel + C5 Laya
- ❌ OTel / Prometheus / 完整 trace（M4）
- ❌ CLI（M5）
- ❌ Tauri（M7）
- ❌ CircuitBreaker / 复杂路由策略（M2-M3 高级部分）
- ❌ capability 探测 / health check 自动摘除
- ❌ 多协议（CORS 浏览器场景）—— MVP 仅 localhost（CORS 已加，但只 very_permissive）
- ❌ trace redact / 隐私配置

---

## 八、当前可复现步骤（用户拿到仓库后）

```bash
# 1. 启动 Laya 本地后端
E:\venvs\laya\Scripts\python.exe E:\tmp\jev_laya_server.py --port 18765 &

# 2. 启动 jev-switch
cd H:\A137442\Develop\AI\Jev\Jev-Switch\rs
export AI_GATEWAY_API_KEY="<your-vercel-key>"
export JEV_SWITCH_CONFIG="$PWD/providers.example.toml"
cargo run

# 3. 启动 React UI
cd H:\A137442\Develop\AI\Jev\Jev-Switch\ui
npm run dev   # http://127.0.0.1:5173

# 4. 浏览器打开 http://127.0.0.1:5173
#    点 4 个 example chips 之一 → 点 Run Jev ↗
#    OUTPUT 应在 <300ms 显示 Laya/Vercel 响应
```

---

## 九、GitHub 仓库

```
URL:    https://github.com/ARCJ137442/jev-switch
Tag:    v0.1.0-mvp  (this commit)
Branch: main
Privacy: private
```

---

## 十、下一步（M1+）

按 ROI 排序：

1. **M1 全 5 类上游**：C1 TypeSafe / C2 OpenRouter / C4 LLM2Jev / C2 sys1 candle —— 新增 3-4 个 Upstream 实现
2. **M2 Router 加固**：FailoverChain + CircuitBreaker + LatencyBased 4 策略
3. **M4 可观测性**：OpenTelemetry + Prometheus + trace viewer 面板
4. **MVP polish**：loading spinner、错误 toast、token 用量表、curl 复制按钮（已部分有）
5. **M7 Tauri 桌面应用**：跨平台打包

每个子任务**可独立派给 subAgent**。

---

**封存完毕。下一版本 v0.2.0-m1 开始 5 类上游扩展。**